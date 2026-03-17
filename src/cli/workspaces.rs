// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Workspace commands — list and manage workspaces.

use super::output;
use super::{ConnOpts, OutputFormat};

/// Run the `workspaces` command — list workspace directories.
pub async fn run_workspaces(conn: &ConnOpts, format: OutputFormat) -> anyhow::Result<()> {
    let client = conn.client();

    let workspaces = match client.get_workspaces().await {
        Ok(w) => w,
        Err(e) if e.is_connection_error() => {
            eprintln!("daemon not running ({})", conn.target());
            std::process::exit(1);
        }
        Err(e) => return Err(e.into()),
    };

    if format == OutputFormat::Json {
        output::print_json(&workspaces);
        return Ok(());
    }

    let entries = workspaces.as_array().cloned().unwrap_or_default();

    if entries.is_empty() {
        println!("No workspaces found.");
        return Ok(());
    }

    let headers = &["name", "path"];
    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|w| {
            vec![
                w.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-")
                    .to_string(),
                w.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-")
                    .to_string(),
            ]
        })
        .collect();
    output::print_table(headers, &rows, format);

    Ok(())
}

/// Run the `workspace` command — show detail or clean.
pub async fn run_workspace(
    identifier: &str,
    clean: bool,
    conn: &ConnOpts,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let client = conn.client();

    if clean {
        if !client.is_running().await {
            eprintln!("daemon not running ({})", conn.target());
            std::process::exit(1);
        }
        println!("Workspace cleanup for '{identifier}' requested.");
        println!(
            "Note: Use the daemon's terminal cleanup or manually remove the workspace directory."
        );
        return Ok(());
    }

    let issue = match client.get_issue(identifier).await {
        Ok(i) => i,
        Err(e) if e.is_connection_error() => {
            eprintln!("daemon not running ({})", conn.target());
            std::process::exit(1);
        }
        Err(e) => return Err(e.into()),
    };

    if format == OutputFormat::Json {
        output::print_json(&issue);
    } else {
        println!("Workspace for: {identifier}");
        println!("{}", "-".repeat(40));
        for (key, value) in issue.as_object().into_iter().flatten() {
            println!("  {:<20} {}", key, value);
        }
    }

    Ok(())
}
