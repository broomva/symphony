//! Start command — launches the Symphony daemon.
//!
//! Extracted from the original `main.rs` `run()` function.

use std::sync::Arc;

use tokio::sync::{Mutex, watch};

use super::StartArgs;

/// Run the Symphony daemon (Algorithm 16.1 entry point).
pub async fn run_start(args: StartArgs, port_override: Option<u16>) -> anyhow::Result<()> {
    let workflow_path = &args.workflow_path;

    // Check if explicit path exists (S17.7: nonexistent explicit path → error)
    if !workflow_path.exists() {
        anyhow::bail!(
            "workflow file not found: {}",
            workflow_path.display()
        );
    }

    tracing::info!(
        workflow = %workflow_path.display(),
        "symphony starting"
    );

    // Load workflow
    let workflow_def = symphony_config::loader::load_workflow(workflow_path)?;
    let config = symphony_config::loader::extract_config(&workflow_def);
    let prompt_template = workflow_def.prompt_template.clone();

    // Validate config
    if let Err(errors) = symphony_config::loader::validate_dispatch_config(&config) {
        for e in &errors {
            tracing::error!(error = %e, "startup validation failed");
        }
        anyhow::bail!("startup validation failed: {}", errors.join("; "));
    }

    let config = Arc::new(config);
    let (config_tx, config_rx) = watch::channel(config.clone());

    // Start workflow watcher
    let watch_path = workflow_path.clone();
    tokio::spawn(async move {
        if let Err(e) = symphony_config::watcher::watch_workflow(watch_path, config_tx).await {
            tracing::error!(%e, "workflow watcher failed");
        }
    });

    // Build tracker client
    let tracker: Arc<dyn symphony_tracker::TrackerClient> = Arc::new(
        symphony_tracker::linear::LinearClient::new(
            config.tracker.endpoint.clone(),
            config.tracker.api_key.clone(),
            config.tracker.project_slug.clone(),
            config.tracker.active_states.clone(),
        ),
    );

    // Build workspace manager
    let workspace_mgr = Arc::new(symphony_workspace::WorkspaceManager::new(
        config.workspace.clone(),
        config.hooks.clone(),
    ));

    // Ensure workspace root exists
    tokio::fs::create_dir_all(&config.workspace.root).await?;

    // Shared observability state
    let obs_state: Arc<Mutex<Option<symphony_core::OrchestratorState>>> =
        Arc::new(Mutex::new(None));

    // Refresh channel for immediate poll trigger
    let (refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);

    // Shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Determine HTTP port (S13.7: CLI overrides config)
    let server_port = port_override.or(config.server_port);

    // Start HTTP server if configured
    if let Some(port) = server_port {
        let server_shutdown_rx = shutdown_rx.clone();
        let app_state = symphony_observability::server::AppState {
            orchestrator: obs_state.clone(),
            refresh_tx: Some(refresh_tx),
            shutdown_tx: Some(Arc::new(shutdown_tx.clone())),
            api_token: std::env::var("SYMPHONY_API_TOKEN").ok().filter(|s| !s.is_empty()),
        };
        tokio::spawn(async move {
            if let Err(e) =
                symphony_observability::server::start_server_with_state(
                    port,
                    app_state,
                    Some(server_shutdown_rx),
                )
                .await
            {
                tracing::error!(%e, "HTTP server failed");
            }
        });
    }

    // Handle SIGINT/SIGTERM for graceful shutdown (S48)
    let sig_shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        tracing::info!("shutdown signal received");
        let _ = sig_shutdown_tx.send(true);
    });

    // Start scheduler with real tracker, workspace manager, and prompt template
    let mut scheduler = symphony_orchestrator::Scheduler::new(
        config,
        config_rx,
        tracker,
        workspace_mgr,
        prompt_template,
        obs_state,
        Some(refresh_rx),
        Some(shutdown_rx),
    );
    scheduler.run().await?;

    tracing::info!("symphony stopped");
    Ok(())
}

/// Wait for shutdown signal (SIGINT or SIGTERM).
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
}

