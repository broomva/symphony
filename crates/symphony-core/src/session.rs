// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Session and run attempt types (Spec Sections 4.1.5, 4.1.6, 4.1.7).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One execution attempt for one issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAttempt {
    pub issue_id: String,
    pub issue_identifier: String,
    /// `None` for first run, `Some(n)` for retries/continuation.
    pub attempt: Option<u32>,
    pub workspace_path: String,
    pub started_at: DateTime<Utc>,
    pub status: RunAttemptStatus,
    pub error: Option<String>,
}

/// Run attempt lifecycle phases (Spec Section 7.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunAttemptStatus {
    PreparingWorkspace,
    BuildingPrompt,
    LaunchingAgentProcess,
    InitializingSession,
    StreamingTurn,
    Finishing,
    Succeeded,
    Failed,
    TimedOut,
    Stalled,
    CanceledByReconciliation,
}

/// State tracked while a coding-agent subprocess is running (Spec Section 4.1.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSession {
    /// `<thread_id>-<turn_id>`
    pub session_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub codex_app_server_pid: Option<String>,
    pub last_codex_event: Option<String>,
    pub last_codex_timestamp: Option<DateTime<Utc>>,
    pub last_codex_message: Option<String>,
    pub codex_input_tokens: u64,
    pub codex_output_tokens: u64,
    pub codex_total_tokens: u64,
    pub last_reported_input_tokens: u64,
    pub last_reported_output_tokens: u64,
    pub last_reported_total_tokens: u64,
    pub turn_count: u32,
}

/// Scheduled retry state for an issue (Spec Section 4.1.7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEntry {
    pub issue_id: String,
    /// Best-effort human ID for status surfaces/logs.
    pub identifier: String,
    /// 1-based for retry queue.
    pub attempt: u32,
    /// Monotonic clock timestamp in ms.
    pub due_at_ms: u64,
    pub error: Option<String>,
}
