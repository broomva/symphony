// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Workflow loader (Spec Sections 5.1, 5.2).
//!
//! Reads WORKFLOW.md, parses YAML front matter and prompt body.

use std::path::Path;

use crate::template::TemplateError;
use crate::types::{AgentConfig, CodexConfig, HooksConfig, ServiceConfig, WorkflowDefinition};

/// Errors from loading a workflow file (Spec Section 5.5).
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("missing_workflow_file: {0}")]
    MissingFile(String),
    #[error("workflow_parse_error: {0}")]
    ParseError(String),
    #[error("workflow_front_matter_not_a_map")]
    FrontMatterNotMap,
    #[error("template_parse_error: {0}")]
    TemplateParse(String),
    #[error("template_render_error: {0}")]
    TemplateRender(String),
}

impl From<TemplateError> for LoadError {
    fn from(e: TemplateError) -> Self {
        match e {
            TemplateError::ParseError(msg) => LoadError::TemplateParse(msg),
            TemplateError::RenderError(msg) => LoadError::TemplateRender(msg),
        }
    }
}

/// Load and parse a WORKFLOW.md file.
pub fn load_workflow(path: &Path) -> Result<WorkflowDefinition, LoadError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| LoadError::MissingFile(format!("{}: {e}", path.display())))?;

    parse_workflow(&content)
}

/// Parse workflow content into definition.
pub fn parse_workflow(content: &str) -> Result<WorkflowDefinition, LoadError> {
    if let Some(after_first) = content.strip_prefix("---") {
        // Find the closing ---
        if let Some(end_idx) = after_first.find("\n---") {
            let yaml_str = &after_first[..end_idx];
            let rest = &after_first[end_idx + 4..]; // skip \n---

            let config: serde_yaml::Value =
                serde_yaml::from_str(yaml_str).map_err(|e| LoadError::ParseError(e.to_string()))?;

            // Front matter must be a map
            if !config.is_mapping() {
                return Err(LoadError::FrontMatterNotMap);
            }

            Ok(WorkflowDefinition {
                config,
                prompt_template: rest.trim().to_string(),
            })
        } else {
            Err(LoadError::ParseError(
                "unclosed front matter delimiter".into(),
            ))
        }
    } else {
        // No front matter
        Ok(WorkflowDefinition {
            config: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            prompt_template: content.trim().to_string(),
        })
    }
}

/// Resolve environment variable references in a string value.
/// `$VAR_NAME` is replaced with the value of the environment variable.
pub fn resolve_env(value: &str) -> String {
    if let Some(var_name) = value.strip_prefix('$') {
        std::env::var(var_name).unwrap_or_default()
    } else {
        value.to_string()
    }
}

/// Expand `~` to home directory in path strings.
pub fn expand_path(value: &str) -> String {
    if let Some(rest) = value.strip_prefix('~')
        && let Some(home) = dirs_path()
    {
        return format!("{home}{rest}");
    }
    value.to_string()
}

fn dirs_path() -> Option<String> {
    std::env::var("HOME").ok()
}

/// Extract a typed ServiceConfig from a WorkflowDefinition.
pub fn extract_config(def: &WorkflowDefinition) -> ServiceConfig {
    let map = def.config.as_mapping().cloned().unwrap_or_default();

    let mut config = ServiceConfig::default();

    // Tracker
    if let Some(tracker) = map.get(serde_yaml::Value::String("tracker".into())) {
        if let Some(kind) = get_str(tracker, "kind") {
            config.tracker.kind = kind;
        }
        if let Some(endpoint) = get_str(tracker, "endpoint") {
            config.tracker.endpoint = resolve_env(&endpoint);
        }
        if let Some(api_key) = get_str(tracker, "api_key") {
            config.tracker.api_key = resolve_env(&api_key);
        }
        if let Some(slug) = get_str(tracker, "project_slug") {
            config.tracker.project_slug = resolve_env(&slug);
        }
        if let Some(done_state) = get_str(tracker, "done_state") {
            config.tracker.done_state = Some(resolve_env(&done_state));
        }
        if let Some(states) = get_string_list(tracker, "active_states") {
            config.tracker.active_states = states;
        }
        if let Some(states) = get_string_list(tracker, "terminal_states") {
            config.tracker.terminal_states = states;
        }
    }

    // Polling
    if let Some(polling) = map.get(serde_yaml::Value::String("polling".into()))
        && let Some(interval) = get_u64(polling, "interval_ms")
    {
        config.polling.interval_ms = interval;
    }

    // Workspace
    if let Some(ws) = map.get(serde_yaml::Value::String("workspace".into()))
        && let Some(root) = get_str(ws, "root")
    {
        let resolved = resolve_env(&root);
        let expanded = expand_path(&resolved);
        config.workspace.root = expanded.into();
    }

    // Hooks
    if let Some(hooks) = map.get(serde_yaml::Value::String("hooks".into())) {
        config.hooks = extract_hooks(hooks);
    }

    // Agent
    if let Some(agent) = map.get(serde_yaml::Value::String("agent".into())) {
        config.agent = extract_agent(agent);
    }

    // Codex
    if let Some(codex) = map.get(serde_yaml::Value::String("codex".into())) {
        config.codex = extract_codex(codex);
    }

    // Server extension
    if let Some(server) = map.get(serde_yaml::Value::String("server".into()))
        && let Some(port) = get_u64(server, "port")
    {
        config.server_port = Some(port as u16);
    }

    // Runtime
    if let Some(runtime) = map.get(serde_yaml::Value::String("runtime".into())) {
        config.runtime = extract_runtime(runtime);
    }

    config
}

fn extract_hooks(v: &serde_yaml::Value) -> HooksConfig {
    let mut hooks = HooksConfig::default();
    if let Some(s) = get_str(v, "after_create") {
        hooks.after_create = Some(s);
    }
    if let Some(s) = get_str(v, "before_run") {
        hooks.before_run = Some(s);
    }
    if let Some(s) = get_str(v, "after_run") {
        hooks.after_run = Some(s);
    }
    if let Some(s) = get_str(v, "before_remove") {
        hooks.before_remove = Some(s);
    }
    if let Some(s) = get_str(v, "pr_feedback") {
        hooks.pr_feedback = Some(s);
    }
    if let Some(timeout) = get_u64(v, "timeout_ms")
        && timeout > 0
    {
        hooks.timeout_ms = timeout;
    }
    hooks
}

fn extract_agent(v: &serde_yaml::Value) -> AgentConfig {
    let mut agent = AgentConfig::default();
    if let Some(n) = get_u64(v, "max_concurrent_agents") {
        agent.max_concurrent_agents = n as u32;
    }
    if let Some(n) = get_u64(v, "max_turns") {
        agent.max_turns = n as u32;
    }
    if let Some(n) = get_u64(v, "max_retry_backoff_ms") {
        agent.max_retry_backoff_ms = n;
    }
    // Per-state concurrency map
    if let Some(by_state) = v.as_mapping().and_then(|m| {
        m.get(serde_yaml::Value::String(
            "max_concurrent_agents_by_state".into(),
        ))
    }) && let Some(mapping) = by_state.as_mapping()
    {
        for (k, val) in mapping {
            if let (Some(state_name), Some(limit)) = (k.as_str(), val.as_u64())
                && limit > 0
            {
                let normalized = state_name.trim().to_lowercase();
                agent
                    .max_concurrent_agents_by_state
                    .insert(normalized, limit as u32);
            }
        }
    }
    agent
}

fn extract_codex(v: &serde_yaml::Value) -> CodexConfig {
    let mut codex = CodexConfig::default();
    if let Some(cmd) = get_str(v, "command") {
        codex.command = resolve_env(&cmd);
    }
    if let Some(s) = get_str(v, "approval_policy") {
        codex.approval_policy = Some(s);
    }
    if let Some(s) = get_str(v, "thread_sandbox") {
        codex.thread_sandbox = Some(s);
    }
    if let Some(s) = get_str(v, "turn_sandbox_policy") {
        codex.turn_sandbox_policy = Some(s);
    }
    if let Some(n) = get_u64(v, "turn_timeout_ms") {
        codex.turn_timeout_ms = n;
    }
    if let Some(n) = get_u64(v, "read_timeout_ms") {
        codex.read_timeout_ms = n;
    }
    if let Some(n) = get_i64(v, "stall_timeout_ms") {
        codex.stall_timeout_ms = n;
    }
    codex
}

fn extract_runtime(v: &serde_yaml::Value) -> crate::types::RuntimeConfig {
    let mut runtime = crate::types::RuntimeConfig::default();
    if let Some(kind) = get_str(v, "kind") {
        runtime.kind = kind;
    }
    if let Some(base_url) = get_str(v, "base_url") {
        runtime.base_url = resolve_env(&base_url);
    }
    if let Some(policy) = v
        .as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String("policy".into())))
    {
        if let Some(allow) = get_string_list(policy, "allow_capabilities") {
            runtime.policy.allow_capabilities = allow;
        }
        if let Some(gate) = get_string_list(policy, "gate_capabilities") {
            runtime.policy.gate_capabilities = gate;
        }
    }
    runtime
}

// YAML value helpers

fn get_str(v: &serde_yaml::Value, key: &str) -> Option<String> {
    v.as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String(key.into())))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn get_u64(v: &serde_yaml::Value, key: &str) -> Option<u64> {
    v.as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String(key.into())))
        .and_then(|v| {
            v.as_u64().or_else(|| {
                v.as_str().and_then(|s| {
                    // Support $ENV_VAR references in numeric fields
                    let resolved = resolve_env(s);
                    resolved.parse().ok()
                })
            })
        })
}

fn get_i64(v: &serde_yaml::Value, key: &str) -> Option<i64> {
    v.as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String(key.into())))
        .and_then(|v| {
            v.as_i64().or_else(|| {
                v.as_str().and_then(|s| {
                    let resolved = resolve_env(s);
                    resolved.parse().ok()
                })
            })
        })
}

fn get_string_list(v: &serde_yaml::Value, key: &str) -> Option<Vec<String>> {
    let val = v
        .as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String(key.into())))?;

    if let Some(seq) = val.as_sequence() {
        Some(
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        )
    } else {
        val.as_str()
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
    }
}

/// Validate the config is sufficient for dispatch (Spec Section 6.3).
pub fn validate_dispatch_config(config: &ServiceConfig) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if config.tracker.kind.is_empty() {
        errors.push("tracker.kind is required".into());
    } else if config.tracker.kind != "linear" && config.tracker.kind != "github" {
        errors.push(format!(
            "unsupported tracker.kind: '{}'",
            config.tracker.kind
        ));
    }

    if config.tracker.api_key.is_empty() {
        errors.push("tracker.api_key is required (after $VAR resolution)".into());
    }

    if (config.tracker.kind == "linear" || config.tracker.kind == "github")
        && config.tracker.project_slug.is_empty()
    {
        errors.push(format!(
            "tracker.project_slug is required for {} tracker",
            config.tracker.kind
        ));
    }

    if config.codex.command.is_empty() {
        errors.push("codex.command must be non-empty".into());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // ── 1.1: WORKFLOW.md parsing ──

    #[test]
    fn parse_workflow_with_front_matter() {
        let content = "---\ntracker:\n  kind: linear\n  project_slug: test-proj\n---\nHello {{ issue.identifier }}!";
        let def = parse_workflow(content).unwrap();
        assert_eq!(def.prompt_template, "Hello {{ issue.identifier }}!");
        assert!(def.config.is_mapping());
    }

    #[test]
    fn parse_workflow_without_front_matter() {
        let content = "Just a prompt body.";
        let def = parse_workflow(content).unwrap();
        assert_eq!(def.prompt_template, "Just a prompt body.");
    }

    #[test]
    fn parse_workflow_non_map_front_matter_fails() {
        let content = "---\n- list item\n---\nbody";
        let err = parse_workflow(content).unwrap_err();
        assert!(matches!(err, LoadError::FrontMatterNotMap));
    }

    #[test]
    fn load_nonexistent_file_returns_missing() {
        let err = load_workflow(std::path::Path::new("/nonexistent/WORKFLOW.md")).unwrap_err();
        assert!(matches!(err, LoadError::MissingFile(_)));
    }

    #[test]
    fn parse_unclosed_front_matter() {
        let content = "---\ntracker:\n  kind: linear";
        let err = parse_workflow(content).unwrap_err();
        assert!(matches!(err, LoadError::ParseError(_)));
    }

    #[test]
    fn parse_invalid_yaml_front_matter() {
        let content = "---\n: : :\n---\nbody";
        let err = parse_workflow(content).unwrap_err();
        assert!(matches!(err, LoadError::ParseError(_)));
    }

    #[test]
    fn prompt_body_trimmed() {
        let content = "---\ntracker:\n  kind: linear\n---\n\n  Hello World  \n\n";
        let def = parse_workflow(content).unwrap();
        assert_eq!(def.prompt_template, "Hello World");
    }

    // ── 1.2: Front matter extraction ──

    #[test]
    fn extract_full_config() {
        let content = r#"---
tracker:
  kind: linear
  endpoint: https://custom.linear.app/graphql
  api_key: test-api-key
  project_slug: my-proj
  active_states:
    - Todo
    - In Progress
  terminal_states:
    - Done
    - Closed
polling:
  interval_ms: 15000
workspace:
  root: /tmp/symphony_test
hooks:
  after_create: "echo created"
  before_run: "echo before"
  after_run: "echo after"
  before_remove: "echo remove"
  pr_feedback: "gh pr view --json comments -q '.comments[].body'"
  timeout_ms: 30000
agent:
  max_concurrent_agents: 5
  max_turns: 10
  max_retry_backoff_ms: 600000
  max_concurrent_agents_by_state:
    todo: 2
    "In Progress": 3
codex:
  command: codex app-server
  approval_policy: auto
  turn_timeout_ms: 1800000
  read_timeout_ms: 10000
  stall_timeout_ms: 120000
server:
  port: 8080
---
Prompt body"#;

        let def = parse_workflow(content).unwrap();
        let config = extract_config(&def);

        // Tracker
        assert_eq!(config.tracker.kind, "linear");
        assert_eq!(config.tracker.endpoint, "https://custom.linear.app/graphql");
        assert_eq!(config.tracker.api_key, "test-api-key");
        assert_eq!(config.tracker.project_slug, "my-proj");
        assert_eq!(config.tracker.active_states, vec!["Todo", "In Progress"]);
        assert_eq!(config.tracker.terminal_states, vec!["Done", "Closed"]);

        // Polling
        assert_eq!(config.polling.interval_ms, 15000);

        // Workspace
        assert_eq!(
            config.workspace.root,
            std::path::PathBuf::from("/tmp/symphony_test")
        );

        // Hooks
        assert_eq!(config.hooks.after_create, Some("echo created".into()));
        assert_eq!(config.hooks.before_run, Some("echo before".into()));
        assert_eq!(config.hooks.after_run, Some("echo after".into()));
        assert_eq!(config.hooks.before_remove, Some("echo remove".into()));
        assert_eq!(
            config.hooks.pr_feedback,
            Some("gh pr view --json comments -q '.comments[].body'".into())
        );
        assert_eq!(config.hooks.timeout_ms, 30000);

        // Agent
        assert_eq!(config.agent.max_concurrent_agents, 5);
        assert_eq!(config.agent.max_turns, 10);
        assert_eq!(config.agent.max_retry_backoff_ms, 600000);
        // Per-state: keys normalized to lowercase
        assert_eq!(
            config.agent.max_concurrent_agents_by_state.get("todo"),
            Some(&2)
        );
        assert_eq!(
            config
                .agent
                .max_concurrent_agents_by_state
                .get("in progress"),
            Some(&3)
        );

        // Codex
        assert_eq!(config.codex.command, "codex app-server");
        assert_eq!(config.codex.approval_policy, Some("auto".into()));
        assert_eq!(config.codex.turn_timeout_ms, 1800000);
        assert_eq!(config.codex.read_timeout_ms, 10000);
        assert_eq!(config.codex.stall_timeout_ms, 120000);

        // Server extension
        assert_eq!(config.server_port, Some(8080));
    }

    #[test]
    fn env_var_resolution_empty_returns_empty() {
        assert_eq!(resolve_env("$SYMPHONY_NONEXISTENT_VAR_XYZ"), "");
    }

    #[test]
    fn env_var_resolution_expands() {
        // SAFETY: test-only, single-threaded test runner context
        unsafe {
            std::env::set_var("SYMPHONY_TEST_KEY", "test-value");
        }
        assert_eq!(resolve_env("$SYMPHONY_TEST_KEY"), "test-value");
        unsafe {
            std::env::remove_var("SYMPHONY_TEST_KEY");
        }
    }

    #[test]
    fn env_var_literal_passthrough() {
        assert_eq!(resolve_env("literal-value"), "literal-value");
    }

    #[test]
    fn polling_interval_string_coercion() {
        let content = "---\npolling:\n  interval_ms: \"5000\"\n---\nbody";
        let def = parse_workflow(content).unwrap();
        let config = extract_config(&def);
        assert_eq!(config.polling.interval_ms, 5000);
    }

    #[test]
    fn hooks_non_positive_timeout_uses_default() {
        let content = "---\nhooks:\n  timeout_ms: 0\n---\nbody";
        let def = parse_workflow(content).unwrap();
        let config = extract_config(&def);
        assert_eq!(config.hooks.timeout_ms, 60000); // default
    }

    #[test]
    fn hooks_negative_timeout_uses_default() {
        let content = "---\nhooks:\n  timeout_ms: -1\n---\nbody";
        let def = parse_workflow(content).unwrap();
        let config = extract_config(&def);
        // -1 won't parse as u64, so default remains
        assert_eq!(config.hooks.timeout_ms, 60000);
    }

    #[test]
    fn per_state_map_normalizes_keys() {
        let content = "---\nagent:\n  max_concurrent_agents_by_state:\n    \"  Todo  \": 2\n    \"IN PROGRESS\": 1\n---\nbody";
        let def = parse_workflow(content).unwrap();
        let config = extract_config(&def);
        assert_eq!(
            config.agent.max_concurrent_agents_by_state.get("todo"),
            Some(&2)
        );
        assert_eq!(
            config
                .agent
                .max_concurrent_agents_by_state
                .get("in progress"),
            Some(&1)
        );
    }

    #[test]
    fn per_state_map_ignores_non_positive() {
        let content =
            "---\nagent:\n  max_concurrent_agents_by_state:\n    todo: 0\n    done: -1\n---\nbody";
        let def = parse_workflow(content).unwrap();
        let config = extract_config(&def);
        assert!(config.agent.max_concurrent_agents_by_state.is_empty());
    }

    #[test]
    fn active_states_csv_parsing() {
        let content = "---\ntracker:\n  active_states: \"Todo, In Progress\"\n---\nbody";
        let def = parse_workflow(content).unwrap();
        let config = extract_config(&def);
        assert_eq!(config.tracker.active_states, vec!["Todo", "In Progress"]);
    }

    #[test]
    fn unknown_keys_ignored() {
        let content = "---\nunknown_key: foobar\ntracker:\n  kind: linear\n---\nbody";
        let def = parse_workflow(content).unwrap();
        let config = extract_config(&def);
        assert_eq!(config.tracker.kind, "linear");
    }

    // ── 1.4: Dispatch preflight validation ──

    #[test]
    fn validate_config_catches_missing_tracker_kind() {
        let config = ServiceConfig::default();
        let errors = validate_dispatch_config(&config).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("tracker.kind")));
    }

    #[test]
    fn validate_config_catches_unsupported_tracker_kind() {
        let config = ServiceConfig {
            tracker: crate::types::TrackerConfig {
                kind: "jira".into(),
                api_key: "key".into(),
                project_slug: "proj".into(),
                ..Default::default()
            },
            codex: CodexConfig {
                command: "codex".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let errors = validate_dispatch_config(&config).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("unsupported")));
    }

    #[test]
    fn validate_config_catches_empty_api_key() {
        let config = ServiceConfig {
            tracker: crate::types::TrackerConfig {
                kind: "linear".into(),
                api_key: String::new(),
                project_slug: "proj".into(),
                ..Default::default()
            },
            codex: CodexConfig {
                command: "codex".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let errors = validate_dispatch_config(&config).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("api_key")));
    }

    #[test]
    fn validate_config_catches_empty_codex_command() {
        let config = ServiceConfig {
            tracker: crate::types::TrackerConfig {
                kind: "linear".into(),
                api_key: "key".into(),
                project_slug: "proj".into(),
                ..Default::default()
            },
            codex: CodexConfig {
                command: String::new(),
                ..Default::default()
            },
            ..Default::default()
        };
        let errors = validate_dispatch_config(&config).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("codex.command")));
    }

    #[test]
    fn validate_config_passes_with_valid() {
        let config = ServiceConfig {
            tracker: crate::types::TrackerConfig {
                kind: "linear".into(),
                api_key: "key".into(),
                project_slug: "proj".into(),
                ..Default::default()
            },
            codex: CodexConfig {
                command: "codex app-server".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(validate_dispatch_config(&config).is_ok());
    }

    #[test]
    fn validate_missing_project_slug_for_linear() {
        let config = ServiceConfig {
            tracker: crate::types::TrackerConfig {
                kind: "linear".into(),
                api_key: "key".into(),
                project_slug: String::new(),
                ..Default::default()
            },
            codex: CodexConfig {
                command: "codex".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let errors = validate_dispatch_config(&config).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("project_slug")));
    }

    #[test]
    fn validate_config_passes_with_github_kind() {
        let config = ServiceConfig {
            tracker: crate::types::TrackerConfig {
                kind: "github".into(),
                api_key: "ghp_token".into(),
                project_slug: "owner/repo".into(),
                ..Default::default()
            },
            codex: CodexConfig {
                command: "codex app-server".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(validate_dispatch_config(&config).is_ok());
    }

    #[test]
    fn validate_missing_project_slug_for_github() {
        let config = ServiceConfig {
            tracker: crate::types::TrackerConfig {
                kind: "github".into(),
                api_key: "ghp_token".into(),
                project_slug: String::new(),
                ..Default::default()
            },
            codex: CodexConfig {
                command: "codex".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let errors = validate_dispatch_config(&config).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("project_slug")));
    }

    // ── 1.6: Error surface ──

    #[test]
    fn error_classes_are_distinct() {
        let e1 = LoadError::MissingFile("x".into());
        let e2 = LoadError::ParseError("x".into());
        let e3 = LoadError::FrontMatterNotMap;
        let e4 = LoadError::TemplateParse("x".into());
        let e5 = LoadError::TemplateRender("x".into());
        // Verify each has a distinct Display string
        let msgs: Vec<String> = vec![e1, e2, e3, e4, e5]
            .into_iter()
            .map(|e| e.to_string())
            .collect();
        assert!(msgs[0].starts_with("missing_workflow_file"));
        assert!(msgs[1].starts_with("workflow_parse_error"));
        assert!(msgs[2].starts_with("workflow_front_matter_not_a_map"));
        assert!(msgs[3].starts_with("template_parse_error"));
        assert!(msgs[4].starts_with("template_render_error"));
    }
}
