//! Poll-and-dispatch scheduler (Spec Section 8.1).
//!
//! Owns the poll tick and coordinates dispatch, reconciliation, and retries.

use std::sync::Arc;

use symphony_config::types::ServiceConfig;
use symphony_core::OrchestratorState;
use tokio::sync::{watch, Mutex};

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

    /// Run the poll loop. This is the main entry point.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("scheduler starting poll loop");

        loop {
            let config = self.config_rx.borrow().clone();

            // Update dynamic config values
            {
                let mut state = self.state.lock().await;
                state.poll_interval_ms = config.polling.interval_ms;
                state.max_concurrent_agents = config.agent.max_concurrent_agents;
            }

            // Run one tick
            self.tick(&config).await;

            // Sleep for poll interval
            let interval = config.polling.interval_ms;
            tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
        }
    }

    /// Execute one poll-and-dispatch tick (Spec Section 16.2).
    async fn tick(&self, config: &ServiceConfig) {
        // 1. Reconcile running issues
        // TODO: reconcile_running_issues

        // 2. Validate dispatch config
        if let Err(errors) = symphony_config::loader::validate_dispatch_config(config) {
            for e in &errors {
                tracing::error!(error = %e, "dispatch config validation failed");
            }
            return;
        }

        // 3. Fetch candidates
        // TODO: fetch from tracker

        // 4. Sort and dispatch
        // TODO: dispatch eligible issues

        tracing::debug!("tick completed");
    }

    /// Get a snapshot of the current orchestrator state.
    pub async fn snapshot(&self) -> OrchestratorState {
        // This clones — acceptable for observability snapshots
        let state = self.state.lock().await;
        // Build a new state with same data
        let mut snapshot = OrchestratorState::new(state.poll_interval_ms, state.max_concurrent_agents);
        snapshot.running = state.running.clone();
        snapshot.claimed = state.claimed.clone();
        snapshot.retry_attempts = state.retry_attempts.clone();
        snapshot.completed = state.completed.clone();
        snapshot.codex_totals = state.codex_totals.clone();
        snapshot.codex_rate_limits = state.codex_rate_limits.clone();
        snapshot
    }
}
