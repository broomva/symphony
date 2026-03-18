// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Poll-and-dispatch scheduler (Spec Sections 8.1, 16.1-16.6).
//!
//! Owns the poll tick and coordinates dispatch, reconciliation, and retries.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use chrono::Utc;
use symphony_agent::{AgentRunner, LinearToolConfig};
use symphony_config::types::ServiceConfig;
use symphony_core::state::RunningEntry;
use symphony_core::{Issue, OrchestratorState, RetryEntry};
use symphony_tracker::TrackerClient;
use symphony_workspace::WorkspaceManager;
use tokio::sync::{Mutex, watch};

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
    shutdown_rx: Option<watch::Receiver<bool>>,
    worker_handles: Arc<StdMutex<HashMap<String, tokio::task::AbortHandle>>>,
    /// Run a single poll cycle then exit.
    once: bool,
    /// Only dispatch these specific ticket identifiers.
    ticket_filter: Option<Vec<String>>,
}

impl Scheduler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        initial_config: Arc<ServiceConfig>,
        config_rx: watch::Receiver<Arc<ServiceConfig>>,
        tracker: Arc<dyn TrackerClient>,
        workspace_mgr: Arc<WorkspaceManager>,
        prompt_template: String,
        obs_state: Arc<Mutex<Option<OrchestratorState>>>,
        refresh_rx: Option<tokio::sync::mpsc::Receiver<()>>,
        shutdown_rx: Option<watch::Receiver<bool>>,
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
            shutdown_rx,
            worker_handles: Arc::new(StdMutex::new(HashMap::new())),
            once: false,
            ticket_filter: None,
        }
    }

    /// Set once mode: run a single poll cycle then exit.
    pub fn set_once(&mut self, once: bool) {
        self.once = once;
    }

    /// Set ticket filter: only dispatch these specific identifiers.
    pub fn set_ticket_filter(&mut self, tickets: Vec<String>) {
        self.ticket_filter = Some(tickets);
    }

    /// Run the poll loop. This is the main entry point (Spec Algorithm 16.1).
    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("scheduler starting poll loop");

        // Startup terminal workspace cleanup (S8.6)
        self.startup_terminal_cleanup().await;

        loop {
            // Check shutdown signal
            if let Some(rx) = &self.shutdown_rx
                && *rx.borrow()
            {
                tracing::info!("shutdown signal received, stopping scheduler");
                break;
            }

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

            // Clean up stale worker abort handles
            self.cleanup_worker_handles().await;

            // Once mode: exit after first tick
            if self.once {
                tracing::info!("once mode: single poll cycle complete");
                break;
            }

            // Sleep for poll interval, but wake early on refresh or shutdown signal
            let interval = config.polling.interval_ms;
            let sleep = tokio::time::sleep(std::time::Duration::from_millis(interval));
            tokio::pin!(sleep);

            match (&mut self.refresh_rx, &mut self.shutdown_rx) {
                (Some(refresh), Some(shutdown)) => {
                    tokio::select! {
                        _ = &mut sleep => {},
                        _ = refresh.recv() => {
                            tracing::info!("refresh signal received, running immediate tick");
                        },
                        _ = shutdown.changed() => {
                            if *shutdown.borrow() {
                                tracing::info!("shutdown signal received during sleep");
                                break;
                            }
                        },
                    }
                }
                (Some(refresh), None) => {
                    tokio::select! {
                        _ = &mut sleep => {},
                        _ = refresh.recv() => {
                            tracing::info!("refresh signal received, running immediate tick");
                        },
                    }
                }
                (None, Some(shutdown)) => {
                    tokio::select! {
                        _ = &mut sleep => {},
                        _ = shutdown.changed() => {
                            if *shutdown.borrow() {
                                tracing::info!("shutdown signal received during sleep");
                                break;
                            }
                        },
                    }
                }
                (None, None) => {
                    sleep.await;
                }
            }
        }

        // Graceful drain: wait for in-flight workers to complete
        self.drain().await;
        tracing::info!("scheduler stopped");
        Ok(())
    }

    /// Drain mode: wait for all in-flight workers to complete, then return.
    async fn drain(&self) {
        loop {
            let running_count = self.state.lock().await.running.len();
            if running_count == 0 {
                tracing::info!("drain complete: all workers finished");
                return;
            }
            tracing::info!(
                running = running_count,
                "draining: waiting for in-flight workers"
            );
            self.publish_snapshot().await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    /// Clean up abort handles for workers that are no longer running.
    async fn cleanup_worker_handles(&self) {
        let state = self.state.lock().await;
        let running_ids: std::collections::HashSet<&str> =
            state.running.keys().map(|s| s.as_str()).collect();
        self.worker_handles
            .lock()
            .unwrap()
            .retain(|id, _| running_ids.contains(id.as_str()));
    }

    /// Execute one poll-and-dispatch tick (Spec Algorithm 16.2).
    async fn tick(&self, config: &ServiceConfig) {
        tracing::debug!("tick: reconciliation phase");
        self.reconcile_running(config).await;

        if let Err(errors) = symphony_config::loader::validate_dispatch_config(config) {
            for e in &errors {
                tracing::error!(error = %e, "dispatch config validation failed");
            }
            return;
        }

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

        // Apply ticket filter if set
        if let Some(ref filter) = self.ticket_filter {
            let before = candidates.len();
            candidates.retain(|issue| {
                filter
                    .iter()
                    .any(|f| issue.identifier.eq_ignore_ascii_case(f))
            });
            tracing::info!(
                before = before,
                after = candidates.len(),
                "applied ticket filter"
            );
        }

        if candidates.is_empty() {
            tracing::debug!("tick: no candidates, skipping dispatch");
            return;
        }

        let state = self.state.lock().await;
        let selected = select_candidates_from(&mut candidates, &state, config);
        drop(state);

        if selected.is_empty() {
            tracing::debug!("tick: no eligible candidates after filtering");
            return;
        }

        tracing::info!(count = selected.len(), "dispatching candidates");

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

        // Stall detection + kill (S8.5 Part A)
        let stalled = {
            let now_ms = Utc::now().timestamp_millis();
            let state = self.state.lock().await;
            reconcile::find_stalled_issues(&state, config.codex.stall_timeout_ms, now_ms)
        };
        for id in &stalled {
            tracing::warn!(issue_id = %id, "killing stalled session");
            if let Some(handle) = self.worker_handles.lock().unwrap().remove(id) {
                handle.abort();
            }
            let mut state = self.state.lock().await;
            if let Some(entry) = state.running.remove(id) {
                state.codex_totals.seconds_running += Utc::now()
                    .signed_duration_since(entry.started_at)
                    .num_seconds() as f64;
                state.codex_totals.input_tokens += entry.codex_input_tokens;
                state.codex_totals.output_tokens += entry.codex_output_tokens;
                state.codex_totals.total_tokens += entry.codex_total_tokens;

                let attempt = entry.retry_attempt.unwrap_or(0) + 1;
                let delay =
                    reconcile::backoff_delay_ms(attempt, config.agent.max_retry_backoff_ms, false);
                state.retry_attempts.insert(
                    id.clone(),
                    RetryEntry {
                        issue_id: id.clone(),
                        identifier: entry.identifier.clone(),
                        attempt,
                        due_at_ms: Utc::now().timestamp_millis() as u64 + delay,
                        error: Some("stalled session killed".into()),
                    },
                );
            }
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
                            if let Some(handle) =
                                self.worker_handles.lock().unwrap().remove(&issue.id)
                            {
                                handle.abort();
                            }
                            let mut state = self.state.lock().await;
                            state.running.remove(&issue.id);
                            state.claimed.remove(&issue.id);
                            drop(state);

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
                            if let Some(handle) =
                                self.worker_handles.lock().unwrap().remove(&issue.id)
                            {
                                handle.abort();
                            }
                            let mut state = self.state.lock().await;
                            state.running.remove(&issue.id);
                            state.claimed.remove(&issue.id);
                        }
                    }
                }
            }
            Err(e) => {
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
    async fn dispatch_and_run(&self, issue: Issue, attempt: Option<u32>, config: &ServiceConfig) {
        let issue_id = issue.id.clone();
        let identifier = issue.identifier.clone();

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

        self.publish_snapshot().await;

        let state = Arc::clone(&self.state);
        let workspace_mgr = Arc::clone(&self.workspace_mgr);
        let prompt_template = Arc::clone(&self.prompt_template);
        let obs_state = Arc::clone(&self.obs_state);
        let tracker = Arc::clone(&self.tracker);
        let config = config.clone();
        let handle_id = issue_id.clone();

        let join_handle = tokio::spawn(async move {
            let result =
                run_worker(&issue, attempt, &config, &workspace_mgr, &prompt_template).await;

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

                // Transition issue to done_state on normal exit (S69)
                if let Some(ref done_state) = config.tracker.done_state {
                    tracing::info!(
                        issue_id = %issue_id,
                        identifier = %identifier,
                        done_state = %done_state,
                        "transitioning issue to done_state"
                    );
                    if let Err(e) = tracker.set_issue_state(&issue_id, done_state).await {
                        // S70: failure is logged but does not block retry scheduling
                        tracing::warn!(
                            issue_id = %issue_id,
                            identifier = %identifier,
                            done_state = %done_state,
                            error = %e,
                            "done_state transition failed, continuing with retry scheduling"
                        );
                    }
                }
            }

            handle_worker_exit(&state, &issue_id, normal_exit, &config).await;

            let snapshot = build_snapshot(&state).await;
            *obs_state.lock().await = Some(snapshot);
        });

        self.worker_handles
            .lock()
            .unwrap()
            .insert(handle_id, join_handle.abort_handle());
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

            {
                let mut state = self.state.lock().await;
                state.retry_attempts.remove(&issue_id);
            }

            match self
                .tracker
                .fetch_issue_states_by_ids(std::slice::from_ref(&issue_id))
                .await
            {
                Ok(issues) if !issues.is_empty() => {
                    let issue = &issues[0];
                    if reconcile::is_active_state(&issue.state, &config.tracker.active_states) {
                        self.dispatch_and_run(issue.clone(), Some(entry.attempt), config)
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
    let mut snapshot = OrchestratorState::new(state.poll_interval_ms, state.max_concurrent_agents);
    snapshot.running = state.running.clone();
    snapshot.claimed = state.claimed.clone();
    snapshot.retry_attempts = state.retry_attempts.clone();
    snapshot.completed = state.completed.clone();
    snapshot.codex_totals = state.codex_totals.clone();
    snapshot.codex_rate_limits = state.codex_rate_limits.clone();

    let now = Utc::now();
    for entry in snapshot.running.values() {
        let elapsed = now.signed_duration_since(entry.started_at).num_seconds() as f64;
        snapshot.codex_totals.seconds_running += elapsed;
    }

    snapshot
}

/// Run a worker for a single issue: workspace -> hooks -> prompt -> agent.
pub async fn run_worker(
    issue: &Issue,
    attempt: Option<u32>,
    config: &ServiceConfig,
    workspace_mgr: &WorkspaceManager,
    prompt_template: &Mutex<String>,
) -> Result<(), anyhow::Error> {
    let workspace = workspace_mgr.create_for_issue(&issue.identifier).await?;
    tracing::info!(
        identifier = %issue.identifier,
        path = %workspace.path.display(),
        created = workspace.created_now,
        "workspace ready"
    );

    workspace_mgr
        .before_run_with_issue(&workspace.path, &issue.identifier, &issue.title)
        .await?;

    let template = prompt_template.lock().await.clone();
    let prompt = symphony_config::template::render_prompt(&template, issue, attempt)
        .map_err(|e| anyhow::anyhow!("prompt render failed: {e}"))?;

    // Branch on runtime kind: "arcan" dispatches via Arcan HTTP, default uses subprocess
    if config.runtime.kind == "arcan" {
        let arcan_config = symphony_arcan::runner::ArcanRuntimeConfig {
            base_url: config.runtime.base_url.clone(),
            policy: if config.runtime.policy.allow_capabilities.is_empty() {
                None
            } else {
                Some(symphony_arcan::runner::ArcanPolicyConfig {
                    allow_capabilities: config.runtime.policy.allow_capabilities.clone(),
                    gate_capabilities: config.runtime.policy.gate_capabilities.clone(),
                })
            },
            timeout_secs: config.codex.turn_timeout_ms / 1000,
        };
        let arcan_runner = symphony_arcan::runner::ArcanAgentRunner::new(arcan_config);
        arcan_runner
            .run_session(
                &workspace.path,
                &prompt,
                &issue.identifier,
                &issue.title,
                attempt,
                config.agent.max_turns,
            )
            .await
            .map_err(|e| anyhow::anyhow!("arcan agent session failed: {e}"))?;
    } else {
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
    }

    workspace_mgr
        .after_run_with_issue(&workspace.path, &issue.identifier, &issue.title)
        .await;

    // PR feedback loop (S59-S62): capture PR review comments for next turn
    let feedback = workspace_mgr
        .pr_feedback(&workspace.path, &issue.identifier, &issue.title)
        .await;
    if !feedback.is_empty() {
        let feedback_path = workspace.path.join(".symphony-pr-feedback.md");
        if let Err(e) = tokio::fs::write(&feedback_path, &feedback).await {
            tracing::warn!(error = %e, "failed to write PR feedback file");
        } else {
            tracing::info!(
                identifier = %issue.identifier,
                bytes = feedback.len(),
                "PR feedback captured for next turn"
            );
        }
    }

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

    let entry = state.running.remove(issue_id);
    if let Some(entry) = &entry {
        let elapsed = Utc::now()
            .signed_duration_since(entry.started_at)
            .num_seconds() as f64;
        state.codex_totals.seconds_running += elapsed;
        state.codex_totals.input_tokens += entry.codex_input_tokens;
        state.codex_totals.output_tokens += entry.codex_output_tokens;
        state.codex_totals.total_tokens += entry.codex_total_tokens;
    }

    let (attempt, delay) = if normal_exit {
        state.completed.insert(issue_id.to_string());
        let delay = reconcile::backoff_delay_ms(1, config.agent.max_retry_backoff_ms, true);
        (1, delay)
    } else {
        let prev_attempt = entry.as_ref().and_then(|e| e.retry_attempt).unwrap_or(0);
        let attempt = prev_attempt + 1;
        let delay = reconcile::backoff_delay_ms(attempt, config.agent.max_retry_backoff_ms, false);
        (attempt, delay)
    };

    let now_ms = Utc::now().timestamp_millis() as u64;
    let identifier = entry
        .as_ref()
        .map(|e| e.identifier.clone())
        .unwrap_or_default();

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
        "worker exit: scheduled retry"
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

    #[test]
    fn done_state_config_is_threaded_through() {
        let mut config = make_config();
        // Verify done_state is None by default
        assert!(config.tracker.done_state.is_none());

        // Set done_state and verify it's accessible
        config.tracker.done_state = Some("Done".into());
        assert_eq!(config.tracker.done_state, Some("Done".into()));
    }

    #[test]
    fn done_state_parsed_from_workflow() {
        let content = r#"---
tracker:
  kind: linear
  api_key: test-key
  project_slug: test-proj
  done_state: Done
---
Prompt body"#;
        let def = symphony_config::loader::parse_workflow(content).unwrap();
        let config = symphony_config::loader::extract_config(&def);
        assert_eq!(config.tracker.done_state, Some("Done".into()));
    }

    #[test]
    fn done_state_absent_from_workflow_is_none() {
        let content = r#"---
tracker:
  kind: linear
  api_key: test-key
  project_slug: test-proj
---
Prompt body"#;
        let def = symphony_config::loader::parse_workflow(content).unwrap();
        let config = symphony_config::loader::extract_config(&def);
        assert!(config.tracker.done_state.is_none());
    }

    // ── Integration: full dispatch cycle with mock tracker ──

    use std::sync::atomic::{AtomicU32, Ordering};

    /// Mock tracker for integration testing.
    struct MockTracker {
        issues: std::sync::Mutex<Vec<Issue>>,
        fetch_count: AtomicU32,
        state_set_calls: std::sync::Mutex<Vec<(String, String)>>,
    }

    impl MockTracker {
        fn new(issues: Vec<Issue>) -> Self {
            Self {
                issues: std::sync::Mutex::new(issues),
                fetch_count: AtomicU32::new(0),
                state_set_calls: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl symphony_tracker::TrackerClient for MockTracker {
        async fn fetch_candidate_issues(
            &self,
        ) -> Result<Vec<Issue>, symphony_tracker::TrackerError> {
            self.fetch_count.fetch_add(1, Ordering::Relaxed);
            Ok(self.issues.lock().unwrap().clone())
        }

        async fn fetch_issues_by_states(
            &self,
            states: &[String],
        ) -> Result<Vec<Issue>, symphony_tracker::TrackerError> {
            let issues = self.issues.lock().unwrap();
            Ok(issues
                .iter()
                .filter(|i| states.iter().any(|s| s.eq_ignore_ascii_case(&i.state)))
                .cloned()
                .collect())
        }

        async fn fetch_issue_states_by_ids(
            &self,
            issue_ids: &[String],
        ) -> Result<Vec<Issue>, symphony_tracker::TrackerError> {
            let issues = self.issues.lock().unwrap();
            Ok(issues
                .iter()
                .filter(|i| issue_ids.contains(&i.id))
                .cloned()
                .collect())
        }

        async fn set_issue_state(
            &self,
            issue_id: &str,
            state: &str,
        ) -> Result<(), symphony_tracker::TrackerError> {
            self.state_set_calls
                .lock()
                .unwrap()
                .push((issue_id.to_string(), state.to_string()));
            Ok(())
        }
    }

    #[test]
    fn mock_tracker_integration_candidate_fetch() {
        let issues = vec![
            make_issue("1", "T-1", Some(2), "Todo"),
            make_issue("2", "T-2", Some(1), "In Progress"),
            make_issue("3", "T-3", None, "Done"),
        ];
        let tracker = MockTracker::new(issues);
        let rt = tokio::runtime::Runtime::new().unwrap();

        let candidates = rt.block_on(tracker.fetch_candidate_issues()).unwrap();
        assert_eq!(candidates.len(), 3);
        assert_eq!(tracker.fetch_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn mock_tracker_integration_full_dispatch_cycle() {
        // Simulate: 3 issues, concurrency=2, verify only 2 dispatched
        let issues = vec![
            make_issue("1", "T-1", Some(1), "Todo"),
            make_issue("2", "T-2", Some(2), "Todo"),
            make_issue("3", "T-3", Some(3), "Todo"),
        ];

        let state = OrchestratorState::new(30000, 2);
        let mut config = make_config();
        config.agent.max_concurrent_agents = 2;

        let tracker = MockTracker::new(issues.clone());
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Step 1: Fetch candidates
        let mut candidates = rt.block_on(tracker.fetch_candidate_issues()).unwrap();
        assert_eq!(candidates.len(), 3);

        // Step 2: Select eligible candidates (respecting concurrency)
        let selected = select_candidates_from(&mut candidates, &state, &config);
        assert_eq!(selected.len(), 2);

        // Step 3: Verify priority ordering (lower priority first)
        assert_eq!(selected[0].identifier, "T-1"); // priority 1
        assert_eq!(selected[1].identifier, "T-2"); // priority 2

        // Step 4: Verify fetch was called
        assert_eq!(tracker.fetch_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn mock_tracker_state_refresh_filters_correctly() {
        let issues = vec![
            make_issue("1", "T-1", Some(1), "Todo"),
            make_issue("2", "T-2", Some(2), "Done"),
        ];
        let tracker = MockTracker::new(issues);
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Refresh only issue "1"
        let refreshed = rt
            .block_on(tracker.fetch_issue_states_by_ids(&["1".to_string()]))
            .unwrap();
        assert_eq!(refreshed.len(), 1);
        assert_eq!(refreshed[0].id, "1");
    }

    #[test]
    fn mock_tracker_terminal_cleanup() {
        let issues = vec![
            make_issue("1", "T-1", Some(1), "Todo"),
            make_issue("2", "T-2", Some(2), "Done"),
            make_issue("3", "T-3", None, "Closed"),
        ];
        let tracker = MockTracker::new(issues);
        let rt = tokio::runtime::Runtime::new().unwrap();

        let terminal = rt
            .block_on(tracker.fetch_issues_by_states(&["Done".to_string(), "Closed".to_string()]))
            .unwrap();
        assert_eq!(terminal.len(), 2);
    }

    #[test]
    fn mock_tracker_done_state_transition() {
        let tracker = MockTracker::new(vec![]);
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(tracker.set_issue_state("issue-1", "Done"))
            .unwrap();
        rt.block_on(tracker.set_issue_state("issue-2", "Closed"))
            .unwrap();

        let calls = tracker.state_set_calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], ("issue-1".to_string(), "Done".to_string()));
        assert_eq!(calls[1], ("issue-2".to_string(), "Closed".to_string()));
    }

    #[test]
    fn dispatch_excludes_terminal_issues() {
        let issues = vec![
            make_issue("1", "T-1", Some(1), "Todo"),
            make_issue("2", "T-2", Some(1), "Done"), // terminal
            make_issue("3", "T-3", Some(1), "In Progress"),
        ];
        let state = OrchestratorState::new(30000, 10);
        let config = make_config();

        let mut candidates = issues;
        let selected = select_candidates_from(&mut candidates, &state, &config);

        // Only active state issues should be selected
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|i| i.state != "Done"));
    }

    #[test]
    fn worker_exit_schedules_retry() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let state = Arc::new(Mutex::new(OrchestratorState::new(30000, 10)));
        let config = make_config();

        // Simulate a running entry
        let issue = make_issue("1", "T-1", Some(1), "Todo");
        rt.block_on(async {
            let mut s = state.lock().await;
            let entry = symphony_core::state::RunningEntry {
                identifier: "T-1".into(),
                issue: issue.clone(),
                session_id: Some("session-1".into()),
                codex_app_server_pid: None,
                last_codex_message: None,
                last_codex_event: None,
                last_codex_timestamp: None,
                codex_input_tokens: 100,
                codex_output_tokens: 200,
                codex_total_tokens: 300,
                last_reported_input_tokens: 0,
                last_reported_output_tokens: 0,
                last_reported_total_tokens: 0,
                retry_attempt: None,
                started_at: Utc::now(),
                turn_count: 1,
            };
            s.running.insert("1".to_string(), entry);
            s.claimed.insert("1".to_string());
        });

        // Handle normal exit
        rt.block_on(handle_worker_exit(&state, "1", true, &config));

        rt.block_on(async {
            let s = state.lock().await;
            // Running entry removed
            assert!(s.running.is_empty());
            // Added to completed
            assert!(s.completed.contains("1"));
            // Retry scheduled
            assert!(s.retry_attempts.contains_key("1"));
            // Tokens accumulated
            assert_eq!(s.codex_totals.total_tokens, 300);
        });
    }
}
