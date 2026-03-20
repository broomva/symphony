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
    /// Runtime configuration for agent execution (subprocess vs. Arcan).
    #[serde(default)]
    pub runtime: RuntimeConfig,
    /// Hive collaborative evolution configuration.
    #[serde(default)]
    pub hive: HiveConfig,
    /// Agent profile — persona, consciousness, skills, control strictness.
    #[serde(default)]
    pub profile: ProfileConfig,
    /// EGRI batch evaluation configuration.
    #[serde(default)]
    pub egri: EgriConfig,
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
    /// After-session hook: runs after all agent work completes (after pr_feedback,
    /// before handle_worker_exit) with session outcome context.
    /// Receives env vars: SYMPHONY_ISSUE_ID, SYMPHONY_ISSUE_TITLE,
    /// SYMPHONY_SESSION_OUTCOME, SYMPHONY_ATTEMPT, SYMPHONY_TOKENS_TOTAL.
    /// Failure is logged and ignored (non-fatal).
    pub after_session: Option<String>,
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

/// Runtime configuration for agent execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    /// Runtime kind: "subprocess" (default) or "arcan"
    #[serde(default = "default_runtime_kind")]
    pub kind: String,
    /// Base URL for the Arcan daemon (only used when kind = "arcan")
    #[serde(default = "default_arcan_url")]
    pub base_url: String,
    /// Policy for Arcan sessions
    #[serde(default)]
    pub policy: RuntimePolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimePolicyConfig {
    #[serde(default)]
    pub allow_capabilities: Vec<String>,
    #[serde(default)]
    pub gate_capabilities: Vec<String>,
}

/// Configuration for hive multi-agent collaborative evolution mode.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HiveConfig {
    /// Whether hive mode is enabled. Issues with "hive" label use multi-agent dispatch.
    #[serde(default)]
    pub enabled: bool,
    /// Number of concurrent agents per hive task.
    #[serde(default = "default_hive_agents")]
    pub agents_per_task: u32,
    /// Maximum number of generations before stopping.
    #[serde(default = "default_hive_generations")]
    pub max_generations: u32,
    /// Stop early if score improvement is below this threshold.
    #[serde(default = "default_convergence_threshold")]
    pub convergence_threshold: f64,
    /// EGRI budget (max trials) per agent per generation.
    #[serde(default = "default_egri_budget")]
    pub egri_budget_per_agent: u32,
    /// Optional script to evaluate artifacts.
    pub eval_script: Option<String>,
    /// Spaces server ID for coordination channels.
    pub spaces_server_id: Option<u64>,
    /// Per-agent profiles for hive mode specialization.
    /// If fewer entries than agents_per_task, remaining agents use the global profile.
    #[serde(default)]
    pub agent_profiles: Vec<ProfileConfig>,
}

/// Configuration for EGRI batch evaluation integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgriConfig {
    /// Enable batch evaluation in the poll loop.
    #[serde(default)]
    pub batch_enabled: bool,
    /// Number of completed sessions between evaluations.
    #[serde(default = "default_eval_batch_size")]
    pub eval_batch_size: u32,
    /// Minimum time (ms) between evaluations.
    #[serde(default = "default_eval_interval_ms")]
    pub eval_interval_ms: u64,
    /// Max trials per evaluation cycle.
    #[serde(default = "default_batch_budget")]
    pub batch_budget: u32,
    /// Autonomy mode: "suggestion", "sandbox", "auto_promote".
    #[serde(default = "default_autonomy")]
    pub autonomy: String,
    /// Path to JSONL ledger file for evaluation records.
    #[serde(default = "default_ledger_path")]
    pub ledger_path: String,
    /// Optional external script to override built-in evaluator.
    pub eval_script: Option<String>,
    /// Minimum score threshold for promotion.
    #[serde(default = "default_score_threshold")]
    pub score_threshold: f64,
    /// Enable Lago-compatible journal events.
    #[serde(default)]
    pub lago_journal: bool,
}

impl Default for EgriConfig {
    fn default() -> Self {
        Self {
            batch_enabled: false,
            eval_batch_size: default_eval_batch_size(),
            eval_interval_ms: default_eval_interval_ms(),
            batch_budget: default_batch_budget(),
            autonomy: default_autonomy(),
            ledger_path: default_ledger_path(),
            eval_script: None,
            score_threshold: default_score_threshold(),
            lago_journal: false,
        }
    }
}

fn default_eval_batch_size() -> u32 {
    5
}
fn default_eval_interval_ms() -> u64 {
    300_000
}
fn default_batch_budget() -> u32 {
    10
}
fn default_autonomy() -> String {
    "sandbox".to_string()
}
fn default_ledger_path() -> String {
    "evals/symphony-prompts/ledger.jsonl".to_string()
}
fn default_score_threshold() -> f64 {
    0.7
}

/// Consciousness depth level for agent profiles.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConsciousnessLevel {
    #[default]
    Baseline,
    Governed,
    Autonomous,
}

impl std::fmt::Display for ConsciousnessLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Baseline => write!(f, "baseline"),
            Self::Governed => write!(f, "governed"),
            Self::Autonomous => write!(f, "autonomous"),
        }
    }
}

/// Control strictness profile for agent execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ControlProfile {
    #[default]
    Baseline,
    Governed,
    Autonomous,
}

impl std::fmt::Display for ControlProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Baseline => write!(f, "baseline"),
            Self::Governed => write!(f, "governed"),
            Self::Autonomous => write!(f, "autonomous"),
        }
    }
}

/// Agent profile configuration — persona, consciousness depth, skills, and control strictness.
/// Fully optional: missing section = all defaults = current behavior.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    /// Agent role description (e.g. "senior Rust engineer").
    #[serde(default)]
    pub role: String,
    /// Consciousness depth level.
    #[serde(default)]
    pub consciousness: ConsciousnessLevel,
    /// Skills the agent has access to.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Control profile strictness.
    #[serde(default)]
    pub control_profile: ControlProfile,
    /// Free-form context text for the agent.
    #[serde(default)]
    pub context: String,
}

fn default_hive_agents() -> u32 {
    3
}
fn default_hive_generations() -> u32 {
    5
}
fn default_convergence_threshold() -> f64 {
    0.01
}
fn default_egri_budget() -> u32 {
    10
}

fn default_runtime_kind() -> String {
    "subprocess".to_string()
}
fn default_arcan_url() -> String {
    "http://localhost:3000".to_string()
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
            after_session: None,
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
