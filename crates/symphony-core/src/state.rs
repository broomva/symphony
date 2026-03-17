// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Orchestrator runtime state (Spec Section 4.1.8).

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::issue::Issue;
use crate::session::RetryEntry;

/// Single authoritative in-memory state owned by the orchestrator.
#[derive(Debug, Serialize, Deserialize)]
pub struct OrchestratorState {
    pub poll_interval_ms: u64,
    pub max_concurrent_agents: u32,
    pub running: HashMap<String, RunningEntry>,
    pub claimed: HashSet<String>,
    pub retry_attempts: HashMap<String, RetryEntry>,
    /// Bookkeeping only, not used for dispatch gating.
    pub completed: HashSet<String>,
    pub codex_totals: CodexTotals,
    pub codex_rate_limits: Option<serde_json::Value>,
}

/// A running entry in the orchestrator state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningEntry {
    pub identifier: String,
    pub issue: Issue,
    pub session_id: Option<String>,
    pub codex_app_server_pid: Option<String>,
    pub last_codex_message: Option<String>,
    pub last_codex_event: Option<String>,
    pub last_codex_timestamp: Option<DateTime<Utc>>,
    pub codex_input_tokens: u64,
    pub codex_output_tokens: u64,
    pub codex_total_tokens: u64,
    pub last_reported_input_tokens: u64,
    pub last_reported_output_tokens: u64,
    pub last_reported_total_tokens: u64,
    pub retry_attempt: Option<u32>,
    pub started_at: DateTime<Utc>,
    pub turn_count: u32,
}

/// Aggregate token and runtime totals.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CodexTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub seconds_running: f64,
}

impl OrchestratorState {
    pub fn new(poll_interval_ms: u64, max_concurrent_agents: u32) -> Self {
        Self {
            poll_interval_ms,
            max_concurrent_agents,
            running: HashMap::new(),
            claimed: HashSet::new(),
            retry_attempts: HashMap::new(),
            completed: HashSet::new(),
            codex_totals: CodexTotals::default(),
            codex_rate_limits: None,
        }
    }

    /// Number of available global dispatch slots.
    pub fn available_slots(&self) -> u32 {
        self.max_concurrent_agents
            .saturating_sub(self.running.len() as u32)
    }

    /// Check if an issue is already claimed (running or retry-queued).
    pub fn is_claimed(&self, issue_id: &str) -> bool {
        self.claimed.contains(issue_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_has_full_slots() {
        let state = OrchestratorState::new(30000, 10);
        assert_eq!(state.available_slots(), 10);
    }

    #[test]
    fn available_slots_decrements() {
        let mut state = OrchestratorState::new(30000, 2);
        state.running.insert(
            "issue-1".into(),
            RunningEntry {
                identifier: "T-1".into(),
                issue: crate::issue::Issue {
                    id: "issue-1".into(),
                    identifier: "T-1".into(),
                    title: "Test".into(),
                    description: None,
                    priority: None,
                    state: "Todo".into(),
                    branch_name: None,
                    url: None,
                    labels: vec![],
                    blocked_by: vec![],
                    created_at: None,
                    updated_at: None,
                },
                session_id: None,
                codex_app_server_pid: None,
                last_codex_message: None,
                last_codex_event: None,
                last_codex_timestamp: None,
                codex_input_tokens: 0,
                codex_output_tokens: 0,
                codex_total_tokens: 0,
                last_reported_input_tokens: 0,
                last_reported_output_tokens: 0,
                last_reported_total_tokens: 0,
                retry_attempt: None,
                started_at: chrono::Utc::now(),
                turn_count: 0,
            },
        );
        assert_eq!(state.available_slots(), 1);
    }
}
