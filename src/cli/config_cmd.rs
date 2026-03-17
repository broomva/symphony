//! Config command — parse and display resolved configuration.

use std::path::Path;

use super::OutputFormat;

/// Run the `config` command — display resolved ServiceConfig.
pub async fn run_config(workflow_path: &Path, format: OutputFormat) -> anyhow::Result<()> {
    // Load workflow
    let workflow_def = symphony_config::loader::load_workflow(workflow_path)?;
    let config = symphony_config::loader::extract_config(&workflow_def);

    if format == OutputFormat::Json {
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }

    // Table display
    println!("Resolved Configuration");
    println!("======================");
    println!("Source: {}", workflow_path.display());
    println!();

    println!("[tracker]");
    println!("  kind:           {}", config.tracker.kind);
    println!("  endpoint:       {}", config.tracker.endpoint);
    println!(
        "  api_key:        {}",
        if config.tracker.api_key.is_empty() {
            "(empty)"
        } else {
            "(set)"
        }
    );
    println!("  project_slug:   {}", config.tracker.project_slug);
    println!("  active_states:  {:?}", config.tracker.active_states);
    println!("  terminal_states:{:?}", config.tracker.terminal_states);
    println!();

    println!("[polling]");
    println!("  interval_ms:    {}", config.polling.interval_ms);
    println!();

    println!("[workspace]");
    println!("  root:           {}", config.workspace.root.display());
    println!();

    println!("[hooks]");
    println!(
        "  after_create:   {}",
        config.hooks.after_create.as_deref().unwrap_or("(none)")
    );
    println!(
        "  before_run:     {}",
        config.hooks.before_run.as_deref().unwrap_or("(none)")
    );
    println!(
        "  after_run:      {}",
        config.hooks.after_run.as_deref().unwrap_or("(none)")
    );
    println!(
        "  before_remove:  {}",
        config.hooks.before_remove.as_deref().unwrap_or("(none)")
    );
    println!("  timeout_ms:     {}", config.hooks.timeout_ms);
    println!();

    println!("[agent]");
    println!(
        "  max_concurrent: {}",
        config.agent.max_concurrent_agents
    );
    println!("  max_turns:      {}", config.agent.max_turns);
    println!(
        "  max_backoff_ms: {}",
        config.agent.max_retry_backoff_ms
    );
    if !config.agent.max_concurrent_agents_by_state.is_empty() {
        println!(
            "  by_state:       {:?}",
            config.agent.max_concurrent_agents_by_state
        );
    }
    println!();

    println!("[codex]");
    println!("  command:        {}", config.codex.command);
    println!(
        "  approval:       {}",
        config
            .codex
            .approval_policy
            .as_deref()
            .unwrap_or("(default)")
    );
    println!("  turn_timeout:   {}ms", config.codex.turn_timeout_ms);
    println!("  read_timeout:   {}ms", config.codex.read_timeout_ms);
    println!("  stall_timeout:  {}ms", config.codex.stall_timeout_ms);
    println!();

    println!("[server]");
    println!(
        "  port:           {}",
        config
            .server_port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "(disabled)".into())
    );

    Ok(())
}
