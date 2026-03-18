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
    workflow_path: &std::path::Path,
    conn: &ConnOpts,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let client = conn.client();

    if clean {
        // Load workflow locally to find workspace root
        match symphony_config::loader::load_workflow(workflow_path) {
            Ok(def) => {
                let config = symphony_config::loader::extract_config(&def);
                let ws_root = config.workspace.root;
                // Normalize identifier for path (same as orchestrator)
                let safe_id: String = identifier
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let ws_path = ws_root.join(&safe_id);
                if ws_path.exists() {
                    std::fs::remove_dir_all(&ws_path)?;
                    println!("Removed workspace: {}", ws_path.display());
                } else {
                    println!("Workspace not found: {}", ws_path.display());
                }
            }
            Err(e) => {
                eprintln!("Cannot resolve workspace root: {e}");
                eprintln!("Provide --workflow-path pointing to your WORKFLOW.md");
                std::process::exit(1);
            }
        }
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
            println!("  {key:<20} {value}");
        }
    }

    Ok(())
}
