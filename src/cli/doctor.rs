// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Doctor command — pre-flight environment check.

use std::path::Path;

use super::ConnOpts;

struct Check {
    name: &'static str,
    passed: bool,
    detail: String,
}

fn check_file_exists(path: &str) -> Check {
    let exists = Path::new(path).exists();
    Check {
        name: "WORKFLOW.md",
        passed: exists,
        detail: if exists {
            format!("found: {path}")
        } else {
            format!("not found: {path}")
        },
    }
}

fn check_workflow_valid(path: &str) -> Check {
    if !Path::new(path).exists() {
        return Check {
            name: "WORKFLOW.md valid",
            passed: false,
            detail: "skipped (file not found)".into(),
        };
    }
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let has_frontmatter = content.starts_with("---");
            Check {
                name: "WORKFLOW.md valid",
                passed: has_frontmatter,
                detail: if has_frontmatter {
                    "has YAML frontmatter".into()
                } else {
                    "missing YAML frontmatter".into()
                },
            }
        }
        Err(e) => Check {
            name: "WORKFLOW.md valid",
            passed: false,
            detail: format!("read error: {e}"),
        },
    }
}

fn check_env_var(name: &str) -> Check {
    let set = std::env::var(name).is_ok_and(|v| !v.is_empty());
    Check {
        name: "env var",
        passed: set,
        detail: if set {
            format!("{name} is set")
        } else {
            format!("{name} is not set")
        },
    }
}

fn check_binary(name: &str) -> Check {
    let found = std::process::Command::new("which")
        .arg(name)
        .output()
        .is_ok_and(|o| o.status.success());
    Check {
        name: "binary",
        passed: found,
        detail: if found {
            format!("{name} found in PATH")
        } else {
            format!("{name} not found in PATH")
        },
    }
}

/// Run the `doctor` command — pre-flight environment check.
pub async fn run_doctor(conn: &ConnOpts) -> anyhow::Result<()> {
    println!("Symphony Doctor");
    println!("===============\n");

    let mut checks: Vec<Check> = Vec::new();

    // 1. WORKFLOW.md
    checks.push(check_file_exists("WORKFLOW.md"));
    checks.push(check_workflow_valid("WORKFLOW.md"));

    // 2. Environment variables
    checks.push(check_env_var("ANTHROPIC_API_KEY"));

    // Check tracker-specific env vars
    let has_linear = std::env::var("LINEAR_API_KEY").is_ok_and(|v| !v.is_empty());
    let has_github = std::env::var("GITHUB_TOKEN").is_ok_and(|v| !v.is_empty());
    checks.push(Check {
        name: "tracker auth",
        passed: has_linear || has_github,
        detail: if has_linear && has_github {
            "LINEAR_API_KEY and GITHUB_TOKEN both set".into()
        } else if has_linear {
            "LINEAR_API_KEY is set".into()
        } else if has_github {
            "GITHUB_TOKEN is set".into()
        } else {
            "neither LINEAR_API_KEY nor GITHUB_TOKEN is set".into()
        },
    });

    // 3. Binaries
    checks.push(check_binary("claude"));
    checks.push(check_binary("gh"));
    checks.push(check_binary("git"));

    // 3b. JS runtime (for dashboard)
    let has_bun = std::process::Command::new("which")
        .arg("bun")
        .output()
        .is_ok_and(|o| o.status.success());
    let has_node = std::process::Command::new("which")
        .arg("node")
        .output()
        .is_ok_and(|o| o.status.success());
    checks.push(Check {
        name: "js runtime",
        passed: has_bun || has_node,
        detail: if has_bun {
            "bun found in PATH".into()
        } else if has_node {
            "node found in PATH (bun recommended)".into()
        } else {
            "neither bun nor node found in PATH".into()
        },
    });

    // 3c. Dashboard directory
    let dashboard_exists = super::dashboard::dashboard_dir_exists();
    let dashboard_path = super::dashboard::expected_dashboard_path();
    checks.push(Check {
        name: "dashboard",
        passed: dashboard_exists,
        detail: if dashboard_exists {
            format!("found: {}", dashboard_path.display())
        } else {
            format!("not found: {}", dashboard_path.display())
        },
    });

    // 4. Daemon connectivity
    let client = conn.client();
    let daemon_running = client.is_running().await;
    checks.push(Check {
        name: "daemon",
        passed: daemon_running,
        detail: if daemon_running {
            format!("reachable at {}", conn.target())
        } else {
            format!("not reachable at {}", conn.target())
        },
    });

    // 5. Workspace root
    let ws_root = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join("symphony-workspaces"))
        .unwrap_or_default();
    let ws_exists = ws_root.exists();
    checks.push(Check {
        name: "workspace root",
        passed: ws_exists,
        detail: if ws_exists {
            format!("exists: {}", ws_root.display())
        } else {
            format!(
                "not found: {} (will be created on first run)",
                ws_root.display()
            )
        },
    });

    // Print results
    let mut pass_count = 0;
    let total = checks.len();
    for check in &checks {
        let icon = if check.passed { "[ok]" } else { "[!!]" };
        if check.passed {
            pass_count += 1;
        }
        println!("  {icon} {:<20} {}", check.name, check.detail);
    }

    println!("\n{pass_count}/{total} checks passed.");
    if pass_count < total {
        println!("Run `symphony doctor` after fixing issues to verify.");
    }

    Ok(())
}
