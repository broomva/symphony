// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Run command — execute a single issue one-shot without the daemon loop.

use std::sync::Arc;

use tokio::sync::Mutex;

use super::RunArgs;

/// Run a single issue one-shot.
pub async fn run_run(args: RunArgs) -> anyhow::Result<()> {
    let workflow_path = &args.workflow_path;

    if !workflow_path.exists() {
        anyhow::bail!("workflow file not found: {}", workflow_path.display());
    }

    // Load workflow
    let workflow_def = symphony_config::loader::load_workflow(workflow_path)?;
    let mut config = symphony_config::loader::extract_config(&workflow_def);
    let prompt_template = workflow_def.prompt_template.clone();

    // Apply CLI overrides
    if let Some(t) = args.turns {
        config.agent.max_turns = t;
    }

    // Build tracker client
    let tracker: Arc<dyn symphony_tracker::TrackerClient> =
        Arc::from(symphony_tracker::create_tracker(&config.tracker)?);

    // Find the specific issue
    eprintln!("Fetching issue {}...", args.identifier);
    let candidates = tracker.fetch_candidate_issues().await?;
    let issue = candidates
        .iter()
        .find(|i| i.identifier.eq_ignore_ascii_case(&args.identifier))
        .cloned();

    let issue = match issue {
        Some(i) => i,
        None => {
            // Try fetching by state refresh (issue might be In Progress, not just Todo)
            let all_issues = tracker
                .fetch_issue_states_by_ids(std::slice::from_ref(&args.identifier))
                .await;
            match all_issues {
                Ok(issues) if !issues.is_empty() => issues[0].clone(),
                _ => {
                    anyhow::bail!(
                        "issue '{}' not found in tracker (must be in active states: {:?})",
                        args.identifier,
                        config.tracker.active_states
                    );
                }
            }
        }
    };

    eprintln!("Running: {} — {}", issue.identifier, issue.title);

    // Build workspace manager
    let workspace_mgr =
        symphony_workspace::WorkspaceManager::new(config.workspace.clone(), config.hooks.clone());

    // Ensure workspace root exists
    tokio::fs::create_dir_all(&config.workspace.root).await?;

    // Run the worker directly
    let prompt_template = Arc::new(Mutex::new(prompt_template));
    let result =
        symphony_orchestrator::run_worker(&issue, None, &config, &workspace_mgr, &prompt_template)
            .await;

    match result {
        Ok(()) => {
            eprintln!("Completed: {}", issue.identifier);
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed: {} — {}", issue.identifier, e);
            Err(e)
        }
    }
}
