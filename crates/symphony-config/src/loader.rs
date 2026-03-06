//! Workflow loader (Spec Sections 5.1, 5.2).
//!
//! Reads WORKFLOW.md, parses YAML front matter and prompt body.

use std::path::Path;

use crate::types::{
    AgentConfig, CodexConfig, HooksConfig, ServiceConfig,
    WorkflowDefinition,
};

/// Errors from loading a workflow file.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("missing_workflow_file: {0}")]
    MissingFile(String),
    #[error("workflow_parse_error: {0}")]
    ParseError(String),
    #[error("workflow_front_matter_not_a_map")]
    FrontMatterNotMap,
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

            let config: serde_yaml::Value = serde_yaml::from_str(yaml_str)
                .map_err(|e| LoadError::ParseError(e.to_string()))?;

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
        && let Some(home) = dirs_path() {
            return format!("{}{rest}", home);
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
            config.tracker.endpoint = endpoint;
        }
        if let Some(api_key) = get_str(tracker, "api_key") {
            config.tracker.api_key = resolve_env(&api_key);
        }
        if let Some(slug) = get_str(tracker, "project_slug") {
            config.tracker.project_slug = slug;
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
        && let Some(interval) = get_u64(polling, "interval_ms") {
            config.polling.interval_ms = interval;
        }

    // Workspace
    if let Some(ws) = map.get(serde_yaml::Value::String("workspace".into()))
        && let Some(root) = get_str(ws, "root") {
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
        && let Some(port) = get_u64(server, "port") {
            config.server_port = Some(port as u16);
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
    if let Some(timeout) = get_u64(v, "timeout_ms")
        && timeout > 0 {
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
    if let Some(by_state) = v
        .as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String("max_concurrent_agents_by_state".into())))
        && let Some(mapping) = by_state.as_mapping() {
            for (k, val) in mapping {
                if let (Some(state_name), Some(limit)) = (k.as_str(), val.as_u64())
                    && limit > 0 {
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
        codex.command = cmd;
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
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
}

fn get_i64(v: &serde_yaml::Value, key: &str) -> Option<i64> {
    v.as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String(key.into())))
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
}

fn get_string_list(v: &serde_yaml::Value, key: &str) -> Option<Vec<String>> {
    let val = v
        .as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String(key.into())))?;

    if let Some(seq) = val.as_sequence() {
        Some(seq.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    } else { val.as_str().map(|s| s.split(',').map(|s| s.trim().to_string()).collect()) }
}

/// Validate the config is sufficient for dispatch (Spec Section 6.3).
pub fn validate_dispatch_config(config: &ServiceConfig) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if config.tracker.kind.is_empty() {
        errors.push("tracker.kind is required".into());
    } else if config.tracker.kind != "linear" {
        errors.push(format!(
            "unsupported tracker.kind: '{}'",
            config.tracker.kind
        ));
    }

    if config.tracker.api_key.is_empty() {
        errors.push("tracker.api_key is required (after $VAR resolution)".into());
    }

    if config.tracker.kind == "linear" && config.tracker.project_slug.is_empty() {
        errors.push("tracker.project_slug is required for linear tracker".into());
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

    #[test]
    fn parse_workflow_with_front_matter() {
        let content = r#"---
tracker:
  kind: linear
  project_slug: test-proj
---
Hello {{ issue.identifier }}!"#;

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
    fn resolve_env_expands_var() {
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
    fn resolve_env_missing_returns_empty() {
        assert_eq!(resolve_env("$SYMPHONY_NONEXISTENT_VAR_XYZ"), "");
    }

    #[test]
    fn resolve_env_literal_passthrough() {
        assert_eq!(resolve_env("literal-value"), "literal-value");
    }

    #[test]
    fn validate_config_catches_missing_tracker() {
        let config = ServiceConfig::default();
        let errors = validate_dispatch_config(&config).unwrap_err();
        assert!(!errors.is_empty());
    }
}
