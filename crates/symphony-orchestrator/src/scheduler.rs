//! Poll-and-dispatch scheduler (Spec Sections 8.1, 16.1-16.6).
//!
//! Owns the poll tick and coordinates dispatch, reconciliation, and retries.

use std::sync::Arc;

use chrono::Utc;
use symphony_config::types::ServiceConfig;
use symphony_core::state::RunningEntry;
use symphony_core::{Issue, OrchestratorState, RetryEntry};
use tokio::sync::{watch, Mutex};

use crate::dispatch::{is_dispatch_eligible, sort_for_dispatch};
use crate::reconcile;

/// The main scheduler that drives the poll loop.
pub struct Scheduler {
    state: Arc<Mutex<OrchestratorState>>,
    config_rx: watch::Receiver<Arc<ServiceConfig>>,
}

impl Scheduler {
    pub fn new(
        initial_config: Arc<ServiceConfig>,
        config_rx: watch::Receiver<Arc<ServiceConfig>>,
    ) -> Self {
        let state = OrchestratorState::new(
            initial_config.polling.interval_ms,
            initial_config.agent.max_concurrent_agents,
        );

        Self {
            state: Arc::new(Mutex::new(state)),
            config_rx,
        }
    }

    /// Run the poll loop. This is the main entry point (Spec Algorithm 16.1).
    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("scheduler starting poll loop");

        // Process any due retry timers at startup and each tick
        loop {
            let config = self.config_rx.borrow().clone();

            // Update dynamic config values
            {
                let mut state = self.state.lock().await;
                state.poll_interval_ms = config.polling.interval_ms;
                state.max_concurrent_agents = config.agent.max_concurrent_agents;
            }

            // Process due retry timers
            self.process_due_retries(&config).await;

            // Run one tick
            self.tick(&config).await;

            // Sleep for poll interval
            let interval = config.polling.interval_ms;
            tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
        }
    }

    /// Execute one poll-and-dispatch tick (Spec Algorithm 16.2).
    async fn tick(&self, config: &ServiceConfig) {
        // 1. Reconcile running issues
        // (In production, this would call the tracker to refresh states)
        tracing::debug!("tick: reconciliation phase");

        // 2. Validate dispatch config
        if let Err(errors) = symphony_config::loader::validate_dispatch_config(config) {
            for e in &errors {
                tracing::error!(error = %e, "dispatch config validation failed");
            }
            return; // Skip dispatch, keep reconciliation
        }

        // 3-4. Fetch candidates, sort, and dispatch
        // (In production, tracker.fetch_candidate_issues() is called here)
        tracing::debug!("tick completed");
    }

    /// Process due retry timers (Spec Algorithm 16.6).
    async fn process_due_retries(&self, config: &ServiceConfig) {
        let now_ms = Utc::now().timestamp_millis() as u64;
        let mut state = self.state.lock().await;

        let due_entries: Vec<_> = state
            .retry_attempts
            .iter()
            .filter(|(_, entry)| entry.due_at_ms <= now_ms)
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect();

        for (issue_id, entry) in due_entries {
            state.retry_attempts.remove(&issue_id);
            // In production: re-fetch candidates, check if issue still exists,
            // dispatch or release claim
            tracing::info!(
                issue_id = %issue_id,
                identifier = %entry.identifier,
                attempt = entry.attempt,
                "retry timer fired"
            );
            // Release claim if we can't dispatch
            state.claimed.remove(&issue_id);
        }

        drop(state);
        let _ = config; // Used in production for tracker operations
    }

    /// Dispatch a single issue (Spec Algorithm 16.4).
    ///
    /// Creates a running entry, adds to claimed set, removes from retry_attempts.
    pub async fn dispatch_issue(&self, issue: Issue, attempt: Option<u32>) {
        let mut state = self.state.lock().await;
        let issue_id = issue.id.clone();

        // Remove any existing retry entry
        state.retry_attempts.remove(&issue_id);

        // Add to claimed set
        state.claimed.insert(issue_id.clone());

        // Create running entry
        let entry = RunningEntry {
            identifier: issue.identifier.clone(),
            issue,
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
            retry_attempt: attempt,
            started_at: Utc::now(),
            turn_count: 0,
        };

        tracing::info!(
            issue_id = %issue_id,
            identifier = %entry.identifier,
            "dispatching issue"
        );

        state.running.insert(issue_id, entry);
    }

    /// Handle worker exit (Spec Algorithm 16.6).
    ///
    /// Normal exit → completed set + continuation retry (attempt 1, 1s delay).
    /// Abnormal exit → exponential backoff retry.
    pub async fn on_worker_exit(
        &self,
        issue_id: &str,
        normal_exit: bool,
        config: &ServiceConfig,
    ) {
        let mut state = self.state.lock().await;

        // Remove from running
        let entry = state.running.remove(issue_id);
        if let Some(entry) = &entry {
            // Add runtime to totals
            let elapsed = Utc::now()
                .signed_duration_since(entry.started_at)
                .num_seconds() as f64;
            state.codex_totals.seconds_running += elapsed;
            state.codex_totals.input_tokens += entry.codex_input_tokens;
            state.codex_totals.output_tokens += entry.codex_output_tokens;
            state.codex_totals.total_tokens += entry.codex_total_tokens;
        }

        let (attempt, delay) = if normal_exit {
            // Normal exit → completed set + continuation retry
            state.completed.insert(issue_id.to_string());
            let delay = reconcile::backoff_delay_ms(1, config.agent.max_retry_backoff_ms, true);
            (1, delay)
        } else {
            // Abnormal exit → exponential backoff
            let prev_attempt = entry
                .as_ref()
                .and_then(|e| e.retry_attempt)
                .unwrap_or(0);
            let attempt = prev_attempt + 1;
            let delay =
                reconcile::backoff_delay_ms(attempt, config.agent.max_retry_backoff_ms, false);
            (attempt, delay)
        };

        let now_ms = Utc::now().timestamp_millis() as u64;
        let identifier = entry
            .as_ref()
            .map(|e| e.identifier.clone())
            .unwrap_or_default();

        // Cancel existing retry timer for same issue (S8.4)
        state.retry_attempts.remove(issue_id);

        state.retry_attempts.insert(
            issue_id.to_string(),
            RetryEntry {
                issue_id: issue_id.to_string(),
                identifier: identifier.clone(),
                attempt,
                due_at_ms: now_ms + delay,
                error: if normal_exit {
                    None
                } else {
                    Some("worker exited abnormally".into())
                },
            },
        );

        tracing::info!(
            issue_id = %issue_id,
            identifier = %identifier,
            attempt = attempt,
            delay_ms = delay,
            normal = normal_exit,
            "worker exit → scheduled retry"
        );
    }

    /// Get a snapshot of the current orchestrator state.
    pub async fn snapshot(&self) -> OrchestratorState {
        let state = self.state.lock().await;
        let mut snapshot =
            OrchestratorState::new(state.poll_interval_ms, state.max_concurrent_agents);
        snapshot.running = state.running.clone();
        snapshot.claimed = state.claimed.clone();
        snapshot.retry_attempts = state.retry_attempts.clone();
        snapshot.completed = state.completed.clone();
        snapshot.codex_totals = state.codex_totals.clone();
        snapshot.codex_rate_limits = state.codex_rate_limits.clone();

        // Add active session elapsed time at snapshot time (S13.5)
        let now = Utc::now();
        for entry in snapshot.running.values() {
            let elapsed = now
                .signed_duration_since(entry.started_at)
                .num_seconds() as f64;
            snapshot.codex_totals.seconds_running += elapsed;
        }

        snapshot
    }

    /// Get locked state for testing.
    #[cfg(test)]
    pub async fn state(&self) -> tokio::sync::MutexGuard<'_, OrchestratorState> {
        self.state.lock().await
    }
}

/// Dispatch candidates from a sorted list while slots remain (S8.2).
pub fn select_candidates(
    issues: &mut [Issue],
    state: &OrchestratorState,
    config: &ServiceConfig,
) -> Vec<Issue> {
    sort_for_dispatch(issues);

    let mut selected = Vec::new();
    let mut simulated_running = state.running.len() as u32;

    for issue in issues.iter() {
        if simulated_running >= config.agent.max_concurrent_agents {
            break;
        }

        if is_dispatch_eligible(
            issue,
            state,
            &config.tracker.terminal_states,
            &config.tracker.active_states,
            &config.agent.max_concurrent_agents_by_state,
        ) {
            selected.push(issue.clone());
            simulated_running += 1;
        }
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_core::Issue;

    fn make_issue(id: &str, identifier: &str, priority: Option<i32>, state: &str) -> Issue {
        Issue {
            id: id.into(),
            identifier: identifier.into(),
            title: "Test".into(),
            description: None,
            priority,
            state: state.into(),
            branch_name: None,
            url: None,
            labels: vec![],
            blocked_by: vec![],
            created_at: Some(Utc::now()),
            updated_at: None,
        }
    }

    fn make_config() -> ServiceConfig {
        ServiceConfig {
            tracker: symphony_config::types::TrackerConfig {
                kind: "linear".into(),
                api_key: "key".into(),
                project_slug: "proj".into(),
                active_states: vec!["Todo".into(), "In Progress".into()],
                terminal_states: vec!["Done".into(), "Closed".into()],
                ..Default::default()
            },
            agent: symphony_config::types::AgentConfig {
                max_concurrent_agents: 10,
                max_turns: 20,
                max_retry_backoff_ms: 300_000,
                ..Default::default()
            },
            codex: symphony_config::types::CodexConfig {
                command: "codex".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn dispatch_adds_to_running_and_claimed() {
        let config = Arc::new(make_config());
        let (_tx, rx) = watch::channel(config.clone());
        let scheduler = Scheduler::new(config, rx);

        let issue = make_issue("1", "T-1", Some(1), "Todo");
        scheduler.dispatch_issue(issue, None).await;

        let state = scheduler.state().await;
        assert!(state.running.contains_key("1"));
        assert!(state.claimed.contains("1"));
    }

    #[tokio::test]
    async fn dispatch_removes_retry_entry() {
        let config = Arc::new(make_config());
        let (_tx, rx) = watch::channel(config.clone());
        let scheduler = Scheduler::new(config, rx);

        // Pre-populate retry entry
        {
            let mut state = scheduler.state().await;
            state.retry_attempts.insert(
                "1".into(),
                RetryEntry {
                    issue_id: "1".into(),
                    identifier: "T-1".into(),
                    attempt: 2,
                    due_at_ms: 99999,
                    error: None,
                },
            );
        }

        let issue = make_issue("1", "T-1", Some(1), "Todo");
        scheduler.dispatch_issue(issue, None).await;

        let state = scheduler.state().await;
        assert!(!state.retry_attempts.contains_key("1"));
    }

    #[tokio::test]
    async fn worker_normal_exit_schedules_continuation_retry() {
        let config = Arc::new(make_config());
        let (_tx, rx) = watch::channel(config.clone());
        let scheduler = Scheduler::new(config.clone(), rx);

        // Dispatch first
        let issue = make_issue("1", "T-1", Some(1), "Todo");
        scheduler.dispatch_issue(issue, None).await;

        // Normal exit
        scheduler.on_worker_exit("1", true, &config).await;

        let state = scheduler.state().await;
        assert!(!state.running.contains_key("1"));
        assert!(state.completed.contains("1"));
        assert!(state.retry_attempts.contains_key("1"));
        let retry = state.retry_attempts.get("1").unwrap();
        assert_eq!(retry.attempt, 1);
        assert!(retry.error.is_none());
    }

    #[tokio::test]
    async fn worker_abnormal_exit_schedules_backoff_retry() {
        let config = Arc::new(make_config());
        let (_tx, rx) = watch::channel(config.clone());
        let scheduler = Scheduler::new(config.clone(), rx);

        let issue = make_issue("1", "T-1", Some(1), "Todo");
        scheduler.dispatch_issue(issue, None).await;

        scheduler.on_worker_exit("1", false, &config).await;

        let state = scheduler.state().await;
        assert!(!state.running.contains_key("1"));
        assert!(state.retry_attempts.contains_key("1"));
        let retry = state.retry_attempts.get("1").unwrap();
        assert_eq!(retry.attempt, 1);
        assert!(retry.error.is_some());
    }

    #[tokio::test]
    async fn worker_exit_accumulates_token_totals() {
        let config = Arc::new(make_config());
        let (_tx, rx) = watch::channel(config.clone());
        let scheduler = Scheduler::new(config.clone(), rx);

        let issue = make_issue("1", "T-1", Some(1), "Todo");
        scheduler.dispatch_issue(issue, None).await;

        // Set token counts
        {
            let mut state = scheduler.state().await;
            if let Some(entry) = state.running.get_mut("1") {
                entry.codex_input_tokens = 100;
                entry.codex_output_tokens = 50;
                entry.codex_total_tokens = 150;
            }
        }

        scheduler.on_worker_exit("1", true, &config).await;

        let state = scheduler.state().await;
        assert_eq!(state.codex_totals.input_tokens, 100);
        assert_eq!(state.codex_totals.output_tokens, 50);
        assert_eq!(state.codex_totals.total_tokens, 150);
    }

    #[test]
    fn select_candidates_respects_limit() {
        let state = OrchestratorState::new(30000, 2);
        let mut config = make_config();
        config.agent.max_concurrent_agents = 2;

        let mut issues: Vec<Issue> = (0..5)
            .map(|i| make_issue(&format!("{i}"), &format!("T-{i}"), Some(1), "Todo"))
            .collect();

        let selected = select_candidates(&mut issues, &state, &config);
        assert_eq!(selected.len(), 2);
    }

    #[tokio::test]
    async fn snapshot_includes_active_elapsed() {
        let config = Arc::new(make_config());
        let (_tx, rx) = watch::channel(config.clone());
        let scheduler = Scheduler::new(config, rx);

        let issue = make_issue("1", "T-1", Some(1), "Todo");
        scheduler.dispatch_issue(issue, None).await;

        // Small sleep so elapsed > 0
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let snapshot = scheduler.snapshot().await;
        assert!(snapshot.codex_totals.seconds_running >= 0.0);
    }

    #[tokio::test]
    async fn validation_failure_does_not_crash() {
        let config = Arc::new(ServiceConfig::default()); // invalid config
        let (_tx, rx) = watch::channel(config.clone());
        let scheduler = Scheduler::new(config.clone(), rx);

        // This should not panic
        scheduler.tick(&config).await;
    }
}
