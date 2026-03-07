//! Symphony - Coding agent orchestration service.
//!
//! A long-running daemon that polls an issue tracker (Linear),
//! creates isolated workspaces per issue, and runs coding agent sessions.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio::sync::{watch, Mutex};

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Check if explicit path exists (S17.7: nonexistent explicit path → error)
    if !cli.workflow_path.exists() {
        eprintln!(
            "error: workflow file not found: {}",
            cli.workflow_path.display()
        );
        std::process::exit(1);
    }

    // Build and run the async runtime
    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(run(cli));

    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    // Initialize logging
    symphony_observability::init_logging();

    tracing::info!(
        workflow = %cli.workflow_path.display(),
        "symphony starting"
    );

    // Load workflow
    let workflow_def = symphony_config::loader::load_workflow(&cli.workflow_path)?;
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
    let watch_path = cli.workflow_path.clone();
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

    // Determine HTTP port (S13.7: CLI overrides config)
    let server_port = cli.port.or(config.server_port);

    // Start HTTP server if configured
    if let Some(port) = server_port {
        let app_state = symphony_observability::server::AppState {
            orchestrator: obs_state.clone(),
            refresh_tx: Some(refresh_tx),
        };
        tokio::spawn(async move {
            if let Err(e) =
                symphony_observability::server::start_server_with_state(port, app_state).await
            {
                tracing::error!(%e, "HTTP server failed");
            }
        });
    }

    // Start scheduler with real tracker, workspace manager, and prompt template
    let mut scheduler = symphony_orchestrator::Scheduler::new(
        config,
        config_rx,
        tracker,
        workspace_mgr,
        prompt_template,
        obs_state,
        Some(refresh_rx),
    );
    scheduler.run().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_valid_workflow() -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(
            f,
            "---\ntracker:\n  kind: linear\n  api_key: test-key\n  project_slug: proj\ncodex:\n  command: echo hi\n---\nPrompt body"
        )
        .unwrap();
        f
    }

    #[test]
    fn cli_default_workflow_path() {
        let cli = Cli::parse_from(["symphony"]);
        assert_eq!(cli.workflow_path, PathBuf::from("WORKFLOW.md"));
        assert!(cli.port.is_none());
    }

    #[test]
    fn cli_explicit_path() {
        let cli = Cli::parse_from(["symphony", "/tmp/custom.md"]);
        assert_eq!(cli.workflow_path, PathBuf::from("/tmp/custom.md"));
    }

    #[test]
    fn cli_port_flag() {
        let f = make_valid_workflow();
        let cli = Cli::parse_from([
            "symphony",
            f.path().to_str().unwrap(),
            "--port",
            "8080",
        ]);
        assert_eq!(cli.port, Some(8080));
    }

    #[test]
    fn cli_port_overrides_config() {
        // CLI --port 8080 should override server.port=3000
        let cli = Cli::parse_from(["symphony", "--port", "8080"]);
        let config_port = Some(3000u16);
        let effective = cli.port.or(config_port);
        assert_eq!(effective, Some(8080));
    }

    #[test]
    fn cli_config_port_used_when_no_flag() {
        let cli = Cli::parse_from(["symphony"]);
        let config_port = Some(3000u16);
        let effective = cli.port.or(config_port);
        assert_eq!(effective, Some(3000));
    }
}
