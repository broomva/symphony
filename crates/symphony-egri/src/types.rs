// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Types for EGRI evaluation records and snapshots.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single evaluation record written to the JSONL ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRecord {
    pub timestamp: DateTime<Utc>,
    pub score: f64,
    pub completed: usize,
    pub retrying: usize,
    pub total_tokens: u64,
    pub total_sessions: usize,
    pub threshold: f64,
    pub passed: bool,
}

/// Snapshot of EGRI evaluation state for the observability API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSnapshot {
    pub last_eval_at: Option<DateTime<Utc>>,
    pub current_score: f64,
    pub total_trials: u32,
    pub promoted_count: u32,
    pub discarded_count: u32,
}

impl Default for EvalSnapshot {
    fn default() -> Self {
        Self {
            last_eval_at: None,
            current_score: 0.0,
            total_trials: 0,
            promoted_count: 0,
            discarded_count: 0,
        }
    }
}

/// Prompt artifact for EGRI mutation tracking (Mode 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArtifact {
    pub template: String,
    pub profile_role: String,
    pub version: u32,
}

/// Hive artifact for multi-agent EGRI (Mode 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveArtifact {
    pub prompt: PromptArtifact,
    pub agent_index: u32,
    pub generation: u32,
    pub score: Option<f64>,
}
