//! Status and stop commands — query/control daemon state.

use super::client::{resolve_port, SymphonyClient};
use super::output;
use super::OutputFormat;

/// Run the `status` command.
pub async fn run_status(port: Option<u16>, format: OutputFormat) -> anyhow::Result<()> {
    let client = SymphonyClient::new(resolve_port(port));

    let state = match client.get_state().await {
        Ok(s) => s,
        Err(e) if e.is_connection_error() => {
            eprintln!("daemon not running (port {})", resolve_port(port));
            std::process::exit(1);
        }
        Err(e) => return Err(e.into()),
    };

    if format == OutputFormat::Json {
        let json = serde_json::to_value(&state)?;
        output::print_json(&json);
        return Ok(());
    }

    println!("Symphony Daemon Status");
    println!("======================");
    output::print_kv("Generated at:", &state.generated_at);
    output::print_kv("Running:", &state.counts.running.to_string());
    output::print_kv("Retrying:", &state.counts.retrying.to_string());
    output::print_kv(
        "Total tokens:",
        &state.codex_totals.total_tokens.to_string(),
    );
    output::print_kv(
        "Runtime:",
        &format!("{:.1}s", state.codex_totals.seconds_running),
    );

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
pub async fn run_stop(port: Option<u16>) -> anyhow::Result<()> {
    let client = SymphonyClient::new(resolve_port(port));

    match client.shutdown().await {
        Ok(resp) => {
            println!(
                "Shutdown requested: {}",
                serde_json::to_string_pretty(&resp)?
            );
            Ok(())
        }
        Err(e) if e.is_connection_error() => {
            eprintln!("daemon not running (port {})", resolve_port(port));
            std::process::exit(1);
        }
        Err(e) => Err(e.into()),
    }
}
