// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Status and stop commands — query/control daemon state.

use super::output;
use super::{ConnOpts, OutputFormat};

/// Run the `status` command.
pub async fn run_status(conn: &ConnOpts, format: OutputFormat) -> anyhow::Result<()> {
    let client = conn.client();

    let state = match client.get_state().await {
        Ok(s) => s,
        Err(e) if e.is_connection_error() => {
            eprintln!("daemon not running ({})", conn.target());
            std::process::exit(1);
        }
        Err(e) => return Err(e.into()),
    };

    // Best-effort metrics fetch (daemon may not support it yet)
    let metrics = client.get_metrics().await.ok();

    if format == OutputFormat::Json {
        let mut json = serde_json::to_value(&state)?;
        if let Some(m) = &metrics {
            json["metrics"] = m.clone();
        }
        output::print_json(&json);
        return Ok(());
    }

    println!("Symphony Daemon Status");
    println!("======================");
    output::print_kv("Generated at:", &state.generated_at);
    output::print_kv("Running:", &state.counts.running.to_string());
    output::print_kv("Retrying:", &state.counts.retrying.to_string());
    output::print_kv(
        "Input tokens:",
        &state.codex_totals.input_tokens.to_string(),
    );
    output::print_kv(
        "Output tokens:",
        &state.codex_totals.output_tokens.to_string(),
    );
    output::print_kv(
        "Total tokens:",
        &state.codex_totals.total_tokens.to_string(),
    );
    output::print_kv(
        "Runtime:",
        &format!("{:.1}s", state.codex_totals.seconds_running),
    );

    // Show config from metrics if available
    if let Some(m) = &metrics
        && let Some(config) = m.get("config")
    {
        if let Some(poll) = config.get("poll_interval_ms") {
            output::print_kv("Poll interval:", &format!("{poll}ms"));
        }
        if let Some(max) = config.get("max_concurrent_agents") {
            output::print_kv("Max concurrent:", &max.to_string());
        }
    }

    if !state.running.is_empty() {
        println!("\nRunning Issues:");
        let headers = &["identifier", "state", "session", "turns", "tokens"];
        let rows: Vec<Vec<String>> = state
            .running
            .iter()
            .map(|r| {
                vec![
                    r.identifier.clone(),
                    r.state.clone(),
                    r.session_id.clone().unwrap_or_else(|| "-".into()),
                    r.turn_count.to_string(),
                    r.tokens.total_tokens.to_string(),
                ]
            })
            .collect();
        output::print_table(headers, &rows, format);
    }

    if !state.retrying.is_empty() {
        println!("\nRetrying Issues:");
        let headers = &["identifier", "attempt", "due_at", "error"];
        let rows: Vec<Vec<String>> = state
            .retrying
            .iter()
            .map(|r| {
                vec![
                    r.identifier.clone(),
                    r.attempt.to_string(),
                    r.due_at_ms.to_string(),
                    r.error.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::print_table(headers, &rows, format);
    }

    Ok(())
}

/// Run the `stop` command.
pub async fn run_stop(conn: &ConnOpts) -> anyhow::Result<()> {
    let client = conn.client();

    match client.shutdown().await {
        Ok(resp) => {
            println!(
                "Shutdown requested: {}",
                serde_json::to_string_pretty(&resp)?
            );
            Ok(())
        }
        Err(e) if e.is_connection_error() => {
            eprintln!("daemon not running ({})", conn.target());
            std::process::exit(1);
        }
        Err(e) => Err(e.into()),
    }
}
