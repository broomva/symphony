// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Batch EGRI runner — periodic evaluation alongside the Symphony poll loop.
//!
//! SymphonyEvaluator computes the resolution rate from orchestrator state:
//!   score = completed / (completed + retrying)
//!
//! BatchEgriRunner triggers evaluation when:
//!   (a) time since last eval >= eval_interval_ms, OR
//!   (b) completed count increased by >= eval_batch_size

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use symphony_config::types::EgriConfig;
use symphony_core::OrchestratorState;
use tokio::sync::Mutex;

use crate::journal::write_eval_record;
use crate::types::{EvalRecord, EvalSnapshot};

/// Evaluates Symphony orchestrator state to produce a resolution score.
pub struct SymphonyEvaluator;

impl SymphonyEvaluator {
    /// Compute resolution rate from orchestrator state.
    /// Returns completed / (completed + retrying), or 0.0 if no data.
    pub fn evaluate(state: &OrchestratorState) -> f64 {
        let completed = state.completed.len();
        let retrying = state.retry_attempts.len();
        let total = completed + retrying;
        if total == 0 {
            return 0.0;
        }
        completed as f64 / total as f64
    }
}

/// Batch EGRI runner — integrates into the scheduler poll loop.
#[derive(Default)]
pub struct BatchEgriRunner {
    last_eval_time_ms: i64,
    last_completed_count: usize,
    egri_state: Arc<Mutex<EvalSnapshot>>,
}

impl BatchEgriRunner {
    pub fn new() -> Self {
        Self {
            last_eval_time_ms: 0,
            last_completed_count: 0,
            egri_state: Arc::new(Mutex::new(EvalSnapshot::default())),
        }
    }

    /// Get the shared EGRI state for observability.
    pub fn state(&self) -> Arc<Mutex<EvalSnapshot>> {
        Arc::clone(&self.egri_state)
    }

    /// Check if evaluation should run and trigger it if conditions are met.
    /// Non-blocking: spawns evaluation as a background tokio task.
    pub async fn maybe_evaluate(
        &mut self,
        state: &Arc<Mutex<Option<OrchestratorState>>>,
        config: &EgriConfig,
    ) {
        if !config.batch_enabled {
            return;
        }

        let now_ms = Utc::now().timestamp_millis();
        let time_elapsed = now_ms - self.last_eval_time_ms;

        // Read current completed count
        let (completed_count, retrying_count, total_tokens) = {
            let guard = state.lock().await;
            if let Some(ref s) = *guard {
                (
                    s.completed.len(),
                    s.retry_attempts.len(),
                    s.codex_totals.total_tokens,
                )
            } else {
                return;
            }
        };

        let batch_triggered = completed_count.saturating_sub(self.last_completed_count)
            >= config.eval_batch_size as usize;
        let time_triggered = time_elapsed >= config.eval_interval_ms as i64;

        if !batch_triggered && !time_triggered {
            return;
        }

        // Compute score
        let total = completed_count + retrying_count;
        let score = if total == 0 {
            0.0
        } else {
            completed_count as f64 / total as f64
        };

        let passed = score >= config.score_threshold;

        let record = EvalRecord {
            timestamp: Utc::now(),
            score,
            completed: completed_count,
            retrying: retrying_count,
            total_tokens,
            total_sessions: total,
            threshold: config.score_threshold,
            passed,
        };

        tracing::info!(
            score = score,
            completed = completed_count,
            retrying = retrying_count,
            passed = passed,
            "EGRI batch evaluation"
        );

        // Update internal state
        self.last_eval_time_ms = now_ms;
        self.last_completed_count = completed_count;

        // Update shared snapshot
        {
            let mut snapshot = self.egri_state.lock().await;
            snapshot.last_eval_at = Some(Utc::now());
            snapshot.current_score = score;
            snapshot.total_trials += 1;
            if passed {
                snapshot.promoted_count += 1;
            } else {
                snapshot.discarded_count += 1;
            }
        }

        // Write to ledger (non-blocking)
        let ledger_path = config.ledger_path.clone();
        tokio::spawn(async move {
            if let Err(e) = write_eval_record(Path::new(&ledger_path), &record).await {
                tracing::warn!(error = %e, "failed to write EGRI ledger record");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_core::state::CodexTotals;

    fn make_state(completed: usize, retrying: usize) -> OrchestratorState {
        let mut state = OrchestratorState::new(30000, 10);
        for i in 0..completed {
            state.completed.insert(format!("c-{i}"));
        }
        for i in 0..retrying {
            state.retry_attempts.insert(
                format!("r-{i}"),
                symphony_core::RetryEntry {
                    issue_id: format!("r-{i}"),
                    identifier: format!("R-{i}"),
                    attempt: 1,
                    due_at_ms: 0,
                    error: None,
                },
            );
        }
        state
    }

    #[test]
    fn evaluator_empty_state_returns_zero() {
        let state = OrchestratorState::new(30000, 10);
        assert_eq!(SymphonyEvaluator::evaluate(&state), 0.0);
    }

    #[test]
    fn evaluator_all_completed() {
        let state = make_state(10, 0);
        assert_eq!(SymphonyEvaluator::evaluate(&state), 1.0);
    }

    #[test]
    fn evaluator_mixed_state() {
        let state = make_state(7, 3);
        assert!((SymphonyEvaluator::evaluate(&state) - 0.7).abs() < 0.001);
    }

    #[test]
    fn evaluator_all_retrying() {
        let state = make_state(0, 5);
        assert_eq!(SymphonyEvaluator::evaluate(&state), 0.0);
    }

    #[tokio::test]
    async fn runner_skips_when_disabled() {
        let mut runner = BatchEgriRunner::new();
        let state = Arc::new(Mutex::new(Some(make_state(10, 0))));
        let config = EgriConfig {
            batch_enabled: false,
            ..Default::default()
        };
        runner.maybe_evaluate(&state, &config).await;
        let snapshot = runner.egri_state.lock().await;
        assert_eq!(snapshot.total_trials, 0);
    }

    #[tokio::test]
    async fn runner_triggers_on_batch_size() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = dir.path().join("ledger.jsonl");

        let mut runner = BatchEgriRunner::new();
        let state = Arc::new(Mutex::new(Some(make_state(6, 4))));
        let config = EgriConfig {
            batch_enabled: true,
            eval_batch_size: 5,
            eval_interval_ms: 999_999_999, // won't trigger by time
            ledger_path: ledger.display().to_string(),
            score_threshold: 0.5,
            ..Default::default()
        };

        runner.maybe_evaluate(&state, &config).await;

        // Wait for background task
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let snapshot = runner.egri_state.lock().await;
        assert_eq!(snapshot.total_trials, 1);
        assert!((snapshot.current_score - 0.6).abs() < 0.001);
        assert_eq!(snapshot.promoted_count, 1); // 0.6 >= 0.5
    }

    #[tokio::test]
    async fn runner_triggers_on_time_interval() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = dir.path().join("ledger.jsonl");

        let mut runner = BatchEgriRunner::new();
        runner.last_eval_time_ms = 0; // long ago
        runner.last_completed_count = 3; // only 1 new completion (below batch_size=5)
        let state = Arc::new(Mutex::new(Some(make_state(4, 1))));
        let config = EgriConfig {
            batch_enabled: true,
            eval_batch_size: 5,
            eval_interval_ms: 1, // will trigger immediately
            ledger_path: ledger.display().to_string(),
            score_threshold: 0.9,
            ..Default::default()
        };

        runner.maybe_evaluate(&state, &config).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let snapshot = runner.egri_state.lock().await;
        assert_eq!(snapshot.total_trials, 1);
        assert_eq!(snapshot.discarded_count, 1); // 0.8 < 0.9
    }
}
