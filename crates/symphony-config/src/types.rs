// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Configuration types (Spec Sections 5.3, 6.4).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Parsed WORKFLOW.md payload (Spec Section 4.1.2).
#[derive(Debug, Clone)]
pub struct WorkflowDefinition {
    /// YAML front matter root object.
    pub config: serde_yaml::Value,
    /// Markdown body after front matter, trimmed.
    pub prompt_template: String,
}

/// Typed runtime values derived from WorkflowDefinition.config (Spec Section 4.1.3).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceConfig {
    pub tracker: TrackerConfig,
    pub polling: PollingConfig,
    pub workspace: WorkspaceConfig,
    pub hooks: HooksConfig,
    pub agent: AgentConfig,
    pub codex: CodexConfig,
    /// Extension: optional HTTP server port.
    pub server_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerConfig {
    pub kind: String,
    pub endpoint: String,
    pub api_key: String,
    pub project_slug: String,
    pub active_states: Vec<String>,
    pub terminal_states: Vec<String>,
    /// State name to transition issues to after successful agent run.
    pub done_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingConfig {
    pub interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    pub after_create: Option<String>,
    pub before_run: Option<String>,
    pub after_run: Option<String>,
    pub before_remove: Option<String>,
    /// PR feedback hook: runs after `after_run` between turns.
    /// Unlike other hooks, its **stdout is captured** and returned as feedback
    /// to use as context for the next agent turn (PR review comments, CI results, etc.).
    /// Failure is logged and ignored (non-fatal).
    pub pr_feedback: Option<String>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_concurrent_agents: u32,
    pub max_turns: u32,
    pub max_retry_backoff_ms: u64,
    pub max_concurrent_agents_by_state: HashMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexConfig {
    pub command: String,
    pub approval_policy: Option<String>,
    pub thread_sandbox: Option<String>,
    pub turn_sandbox_policy: Option<String>,
    pub turn_timeout_ms: u64,
    pub read_timeout_ms: u64,
    pub stall_timeout_ms: i64,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            kind: String::new(),
            endpoint: "https://api.linear.app/graphql".into(),
            api_key: String::new(),
            project_slug: String::new(),
            active_states: vec!["Todo".into(), "In Progress".into()],
            terminal_states: vec![
                "Closed".into(),
                "Cancelled".into(),
                "Canceled".into(),
                "Duplicate".into(),
                "Done".into(),
            ],
            done_state: None,
        }
    }
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self { interval_ms: 30000 }
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            root: std::env::temp_dir().join("symphony_workspaces"),
        }
    }
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            after_create: None,
            before_run: None,
            after_run: None,
            before_remove: None,
            pr_feedback: None,
            timeout_ms: 60000,
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: 10,
            max_turns: 20,
            max_retry_backoff_ms: 300_000,
            max_concurrent_agents_by_state: HashMap::new(),
        }
    }
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            command: "codex app-server".into(),
            approval_policy: None,
            thread_sandbox: None,
            turn_sandbox_policy: None,
            turn_timeout_ms: 3_600_000,
            read_timeout_ms: 5000,
            stall_timeout_ms: 300_000,
        }
    }
}
