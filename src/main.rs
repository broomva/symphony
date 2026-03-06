//! Symphony - Coding agent orchestration service.
//!
//! A long-running daemon that polls an issue tracker (Linear),
//! creates isolated workspaces per issue, and runs coding agent sessions.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio::sync::watch;

/// Symphony: orchestrate coding agents for project work.
#[derive(Parser, Debug)]
#[command(name = "symphony", version, about)]
struct Cli {
    /// Path to WORKFLOW.md file.
    #[arg(default_value = "WORKFLOW.md")]
    workflow_path: PathBuf,

    /// HTTP server port (overrides server.port in WORKFLOW.md).
    #[arg(long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    symphony_observability::init_logging();

    tracing::info!(
        workflow = %cli.workflow_path.display(),
        "symphony starting"
    );

    // Load workflow
    let workflow_def = symphony_config::loader::load_workflow(&cli.workflow_path)?;
    let config = symphony_config::loader::extract_config(&workflow_def);

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
    let watch_path = cli.workflow_path.clone();
    tokio::spawn(async move {
        if let Err(e) = symphony_config::watcher::watch_workflow(watch_path, config_tx).await {
            tracing::error!(%e, "workflow watcher failed");
        }
    });

    // Determine HTTP port
    let server_port = cli.port.or(config.server_port);

    // Start HTTP server if configured
    if let Some(port) = server_port {
        tokio::spawn(async move {
            if let Err(e) = symphony_observability::server::start_server(port).await {
                tracing::error!(%e, "HTTP server failed");
            }
        });
    }

    // Start scheduler
    let mut scheduler = symphony_orchestrator::Scheduler::new(config, config_rx);
    scheduler.run().await?;

    Ok(())
}
