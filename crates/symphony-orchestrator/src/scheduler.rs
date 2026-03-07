//! Poll-and-dispatch scheduler (Spec Sections 8.1, 16.1-16.6).
//!
//! Owns the poll tick and coordinates dispatch, reconciliation, and retries.

use std::sync::Arc;

use chrono::Utc;
use symphony_agent::{AgentRunner, LinearToolConfig};
use symphony_config::types::ServiceConfig;
use symphony_core::state::RunningEntry;
use symphony_core::{Issue, OrchestratorState, RetryEntry};
use symphony_tracker::TrackerClient;
use symphony_workspace::WorkspaceManager;
use tokio::sync::{watch, Mutex};

use crate::dispatch::{is_dispatch_eligible, sort_for_dispatch};
use crate::reconcile;

/// The main scheduler that drives the poll loop.
pub struct Scheduler {
    state: Arc<Mutex<OrchestratorState>>,
    config_rx: watch::Receiver<Arc<ServiceConfig>>,
    tracker: Arc<dyn TrackerClient>,
    workspace_mgr: Arc<WorkspaceManager>,
    prompt_template: Arc<Mutex<String>>,
    obs_state: Arc<Mutex<Option<OrchestratorState>>>,
    refresh_rx: Option<tokio::sync::mpsc::Receiver<()>>,
}

impl Scheduler {
    pub fn new(
        initial_config: Arc<ServiceConfig>,
        config_rx: watch::Receiver<Arc<ServiceConfig>>,
        tracker: Arc<dyn TrackerClient>,
        workspace_mgr: Arc<WorkspaceManager>,
        prompt_template: String,
        obs_state: Arc<Mutex<Option<OrchestratorState>>>,
        refresh_rx: Option<tokio::sync::mpsc::Receiver<()>>,
    ) -> Self {
        let state = OrchestratorState::new(
            initial_config.polling.interval_ms,
            initial_config.agent.max_concurrent_agents,
        );

        Self {
            state: Arc::new(Mutex::new(state)),
            config_rx,
            tracker,
            workspace_mgr,
            prompt_template: Arc::new(Mutex::new(prompt_template)),
            obs_state,
            refresh_rx,
        }
    }

    /// Run the poll loop. This is the main entry point (Spec Algorithm 16.1).
    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("scheduler starting poll loop");

        // Startup terminal workspace cleanup (S8.6)
        self.startup_terminal_cleanup().await;

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

            // Publish state snapshot to observability server
            self.publish_snapshot().await;

            // Sleep for poll interval, but wake early on refresh signal
            let interval = config.polling.interval_ms;
            let sleep = tokio::time::sleep(std::time::Duration::from_millis(interval));
            tokio::pin!(sleep);

            if let Some(rx) = &mut self.refresh_rx {
                tokio::select! {
                    _ = &mut sleep => {},
                    _ = rx.recv() => {
                        tracing::info!("refresh signal received, running immediate tick");
                    },
                }
            } else {
                sleep.await;
            }
        }
    }

    /// Execute one poll-and-dispatch tick (Spec Algorithm 16.2).
    async fn tick(&self, config: &ServiceConfig) {
        // 1. Reconcile running issues
        tracing::debug!("tick: reconciliation phase");
        self.reconcile_running(config).await;

        // 2. Validate dispatch config
        if let Err(errors) = symphony_config::loader::validate_dispatch_config(config) {
            for e in &errors {
                tracing::error!(error = %e, "dispatch config validation failed");
            }
            return; // Skip dispatch, keep reconciliation
        }

        // 3. Fetch candidate issues from tracker
        tracing::debug!("tick: fetching candidates");
        let mut candidates = match self.tracker.fetch_candidate_issues().await {
            Ok(issues) => {
                tracing::info!(count = issues.len(), "fetched candidate issues");
                issues
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to fetch candidates");
                return;
            }
        };

        if candidates.is_empty() {
            tracing::debug!("tick: no candidates, skipping dispatch");
            return;
        }

        // 4. Sort and select eligible candidates
        let state = self.state.lock().await;
        let selected = select_candidates_from(&mut candidates, &state, config);
        drop(state);

        if selected.is_empty() {
            tracing::debug!("tick: no eligible candidates after filtering");
            return;
        }

        tracing::info!(count = selected.len(), "dispatching candidates");

        // 5. Dispatch each selected issue
        for issue in selected {
            self.dispatch_and_run(issue, None, config).await;
        }

        tracing::debug!("tick completed");
    }

    /// Reconcile running issues by refreshing their states from the tracker (S8.5).
    async fn reconcile_running(&self, config: &ServiceConfig) {
        let running_ids: Vec<String> = {
            let state = self.state.lock().await;
            if state.running.is_empty() {
                return;
            }
            state.running.keys().cloned().collect()
        };

        // Stall detection (S8.5 Part A)
        let now_ms = Utc::now().timestamp_millis();
        {
            let state = self.state.lock().await;
            let stalled = reconcile::find_stalled_issues(
                &state,
                config.codex.stall_timeout_ms,
                now_ms,
            );
            for id in &stalled {
                tracing::warn!(issue_id = %id, "stalled session detected");
            }
            // TODO: kill stalled processes and retry
        }

        // Refresh issue states from tracker (S8.5 Part B)
        match self.tracker.fetch_issue_states_by_ids(&running_ids).await {
            Ok(refreshed) => {
                for issue in refreshed {
                    let action = reconcile::reconcile_action(
                        &issue.state,
                        &config.tracker.active_states,
                        &config.tracker.terminal_states,
                    );
                    match action {
                        reconcile::ReconcileAction::UpdateSnapshot => {
                            // Update in-memory snapshot with new state
                            let mut state = self.state.lock().await;
                            if let Some(entry) = state.running.get_mut(&issue.id) {
                                entry.issue.state = issue.state;
                            }
                        }
                        reconcile::ReconcileAction::TerminateAndClean => {
                            tracing::info!(
                                issue_id = %issue.id,
                                identifier = %issue.identifier,
                                state = %issue.state,
                                "issue moved to terminal state, cleaning up"
                            );
                            let mut state = self.state.lock().await;
                            state.running.remove(&issue.id);
                            state.claimed.remove(&issue.id);
                            drop(state);

                            // Clean workspace (S8.5)
                            if let Err(e) = self.workspace_mgr.clean(&issue.identifier).await {
                                tracing::warn!(error = %e, "workspace cleanup failed");
                            }
                        }
                        reconcile::ReconcileAction::TerminateNoCleanup => {
                            tracing::info!(
                                issue_id = %issue.id,
                                identifier = %issue.identifier,
                                state = %issue.state,
                                "issue neither active nor terminal, releasing"
                            );
                            let mut state = self.state.lock().await;
                            state.running.remove(&issue.id);
                            state.claimed.remove(&issue.id);
                        }
                    }
                }
            }
            Err(e) => {
                // S8.5: Refresh failure → keep workers running
                tracing::warn!(error = %e, "state refresh failed, keeping workers");
            }
        }
    }

    /// Startup terminal workspace cleanup (S8.6).
    async fn startup_terminal_cleanup(&self) {
        let config = self.config_rx.borrow().clone();
        match self
            .tracker
            .fetch_issues_by_states(&config.tracker.terminal_states)
            .await
        {
            Ok(terminal_issues) => {
                for issue in &terminal_issues {
                    if let Err(e) = self.workspace_mgr.clean(&issue.identifier).await {
                        // S8.6: Terminal fetch failure → log warning, continue
                        tracing::warn!(
                            identifier = %issue.identifier,
                            error = %e,
                            "startup cleanup failed"
                        );
                    }
                }
                if !terminal_issues.is_empty() {
                    tracing::info!(
                        count = terminal_issues.len(),
                        "startup terminal cleanup complete"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "startup terminal fetch failed, continuing");
            }
        }
    }

    /// Dispatch a single issue: claim, create workspace, render prompt, spawn agent (S16.4).
    async fn dispatch_and_run(
        &self,
        issue: Issue,
        attempt: Option<u32>,
        config: &ServiceConfig,
    ) {
        let issue_id = issue.id.clone();
        let identifier = issue.identifier.clone();

        // Add to running + claimed
        {
            let mut state = self.state.lock().await;
            state.claimed.insert(issue_id.clone());
            let entry = RunningEntry {
                identifier: identifier.clone(),
                issue: issue.clone(),
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
                identifier = %identifier,
                "dispatching issue"
            );
            state.running.insert(issue_id.clone(), entry);
        }

        // Publish state immediately so dashboard shows it
        self.publish_snapshot().await;

        // Spawn worker task
        let state = Arc::clone(&self.state);
        let workspace_mgr = Arc::clone(&self.workspace_mgr);
        let prompt_template = Arc::clone(&self.prompt_template);
        let obs_state = Arc::clone(&self.obs_state);
        let config = config.clone();

        tokio::spawn(async move {
            let result = run_worker(
                &issue,
                attempt,
                &config,
                &workspace_mgr,
                &prompt_template,
            )
            .await;

            let normal_exit = result.is_ok();

            if let Err(ref e) = result {
                tracing::error!(
                    issue_id = %issue_id,
                    identifier = %identifier,
                    error = %e,
                    "worker failed"
                );
            } else {
                tracing::info!(
                    issue_id = %issue_id,
                    identifier = %identifier,
                    "worker completed normally"
                );
            }

            // Handle worker exit: update state, schedule retry
            handle_worker_exit(&state, &issue_id, normal_exit, &config).await;

            // Update observability
            let snapshot = build_snapshot(&state).await;
            *obs_state.lock().await = Some(snapshot);
        });
    }

    /// Process due retry timers (Spec Algorithm 16.6).
    async fn process_due_retries(&self, config: &ServiceConfig) {
        let now_ms = Utc::now().timestamp_millis() as u64;
        let due_entries: Vec<(String, RetryEntry)> = {
            let state = self.state.lock().await;
            state
                .retry_attempts
                .iter()
                .filter(|(_, entry)| entry.due_at_ms <= now_ms)
                .map(|(id, entry)| (id.clone(), entry.clone()))
                .collect()
        };

        for (issue_id, entry) in due_entries {
            tracing::info!(
                issue_id = %issue_id,
                identifier = %entry.identifier,
                attempt = entry.attempt,
                "retry timer fired"
            );

            // Remove retry entry
            {
                let mut state = self.state.lock().await;
                state.retry_attempts.remove(&issue_id);
            }

            // Re-fetch the issue to check if it's still eligible
            match self
                .tracker
                .fetch_issue_states_by_ids(std::slice::from_ref(&issue_id))
                .await
            {
                Ok(issues) if !issues.is_empty() => {
                    let issue = &issues[0];
                    if reconcile::is_active_state(
                        &issue.state,
                        &config.tracker.active_states,
                    ) {
                        // Still active → re-dispatch with retry attempt
                        self.dispatch_and_run(
                            issue.clone(),
                            Some(entry.attempt),
                            config,
                        )
                        .await;
                    } else {
                        tracing::info!(
                            issue_id = %issue_id,
                            state = %issue.state,
                            "retry: issue no longer active, releasing"
                        );
                        let mut state = self.state.lock().await;
                        state.claimed.remove(&issue_id);
                    }
                }
                Ok(_) => {
                    // Issue not found, release claim
                    let mut state = self.state.lock().await;
                    state.claimed.remove(&issue_id);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "retry re-fetch failed, releasing claim");
                    let mut state = self.state.lock().await;
                    state.claimed.remove(&issue_id);
                }
            }
        }
    }

    /// Publish a state snapshot to the observability server.
    async fn publish_snapshot(&self) {
        let snapshot = build_snapshot(&self.state).await;
        *self.obs_state.lock().await = Some(snapshot);
    }

    /// Get a snapshot of the current orchestrator state.
    pub async fn snapshot(&self) -> OrchestratorState {
        build_snapshot(&self.state).await
    }

    /// Get locked state for testing.
    #[cfg(test)]
    pub async fn state(&self) -> tokio::sync::MutexGuard<'_, OrchestratorState> {
        self.state.lock().await
    }
}

/// Build a snapshot of the orchestrator state (S13.5).
async fn build_snapshot(state: &Arc<Mutex<OrchestratorState>>) -> OrchestratorState {
    let state = state.lock().await;
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

/// Run a worker for a single issue: workspace → hooks → prompt → agent.
async fn run_worker(
    issue: &Issue,
    attempt: Option<u32>,
    config: &ServiceConfig,
    workspace_mgr: &WorkspaceManager,
    prompt_template: &Mutex<String>,
) -> Result<(), anyhow::Error> {
    // 1. Create/reuse workspace (S9.1-9.2)
    let workspace = workspace_mgr.create_for_issue(&issue.identifier).await?;
    tracing::info!(
        identifier = %issue.identifier,
        path = %workspace.path.display(),
        created = workspace.created_now,
        "workspace ready"
    );

    // 2. Run before_run hook (S9.4: failure = fatal to attempt)
    workspace_mgr
        .before_run_with_id(&workspace.path, &issue.identifier)
        .await?;

    // 3. Render prompt (S12)
    let template = prompt_template.lock().await.clone();
    let prompt = symphony_config::template::render_prompt(&template, issue, attempt)
        .map_err(|e| anyhow::anyhow!("prompt render failed: {e}"))?;

    // 4. Launch agent (S10.1-10.6)
    let runner = if config.tracker.kind == "linear" {
        AgentRunner::with_linear_tool(
            config.codex.clone(),
            LinearToolConfig {
                endpoint: config.tracker.endpoint.clone(),
                api_key: config.tracker.api_key.clone(),
            },
        )
    } else {
        AgentRunner::new(config.codex.clone())
    };

    let identifier = issue.identifier.clone();
    let max_turns = config.agent.max_turns;

    let on_event: symphony_agent::EventCallback = Box::new(move |event| {
        tracing::info!(
            identifier = %identifier,
            event = ?event,
            "agent event"
        );
    });

    // Use simple (pipe) mode for CLI agents, JSON-RPC mode for app-servers
    let is_app_server = config.codex.command.contains("app-server");
    if is_app_server {
        runner
            .run_session(
                &workspace.path,
                &prompt,
                &issue.identifier,
                &issue.title,
                attempt,
                max_turns,
                &on_event,
            )
            .await
            .map_err(|e| anyhow::anyhow!("agent session failed: {e}"))?;
    } else {
        runner
            .run_simple_session(
                &workspace.path,
                &prompt,
                &issue.identifier,
                &issue.title,
                attempt,
                max_turns,
                &on_event,
            )
            .await
            .map_err(|e| anyhow::anyhow!("agent session failed: {e}"))?;
    }

    // 5. Run after_run hook (S9.4: failure logged and ignored)
    workspace_mgr
        .after_run_with_id(&workspace.path, &issue.identifier)
        .await;

    Ok(())
}

/// Handle worker exit: accumulate tokens, schedule retry (S16.6).
async fn handle_worker_exit(
    state: &Arc<Mutex<OrchestratorState>>,
    issue_id: &str,
    normal_exit: bool,
    config: &ServiceConfig,
) {
    let mut state = state.lock().await;

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

/// Dispatch candidates from a sorted list while slots remain (S8.2).
fn select_candidates_from(
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

/// Public alias for backward compatibility.
pub fn select_candidates(
    issues: &mut [Issue],
    state: &OrchestratorState,
    config: &ServiceConfig,
) -> Vec<Issue> {
    select_candidates_from(issues, state, config)
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

    #[test]
    fn select_candidates_respects_limit() {
        let state = OrchestratorState::new(30000, 2);
        let mut config = make_config();
        config.agent.max_concurrent_agents = 2;

        let mut issues: Vec<Issue> = (0..5)
            .map(|i| make_issue(&format!("{i}"), &format!("T-{i}"), Some(1), "Todo"))
            .collect();

        let selected = select_candidates_from(&mut issues, &state, &config);
        assert_eq!(selected.len(), 2);
    }
}
