// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Dashboard lifecycle — find, configure, and spawn the Next.js dashboard.

use std::path::{Path, PathBuf};

use rand::Rng;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use super::DashboardCommand;

/// Handle to a running dashboard process.
///
/// Killing the child on [`Drop`] ensures cleanup even on panics.
pub struct DashboardHandle {
    child: Option<Child>,
    /// The port the dashboard is listening on.
    pub port: u16,
}

impl DashboardHandle {
    /// Gracefully kill the dashboard process.
    pub fn shutdown(&mut self) {
        if let Some(ref mut child) = self.child.take() {
            let pid = child.id();
            if let Err(e) = child.start_kill() {
                tracing::warn!(?pid, %e, "failed to kill dashboard process");
            } else {
                tracing::info!(?pid, "dashboard process killed");
            }
        }
    }
}

impl Drop for DashboardHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Route `symphony dashboard <subcommand>`.
pub async fn run_dashboard_command(cmd: DashboardCommand) -> anyhow::Result<()> {
    match cmd {
        DashboardCommand::Install => {
            let path = expected_dashboard_path();
            println!("Dashboard expected at: {}", path.display());
            if path.is_dir() {
                println!("  Status: found");
            } else {
                println!("  Status: not found");
                println!(
                    "\nClone or copy the dashboard into this location, \
                     or set SYMPHONY_DASHBOARD_PATH."
                );
            }
            Ok(())
        }
        DashboardCommand::Status(args) => {
            let running = is_running(args.port).await;
            if running {
                println!("Dashboard is running on port {}", args.port);
            } else {
                println!("Dashboard is not running on port {}", args.port);
            }
            Ok(())
        }
        DashboardCommand::Stop(args) => {
            stop(args.port).await?;
            println!("Dashboard on port {} stopped", args.port);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Launch the Next.js dashboard alongside the daemon.
///
/// Returns a [`DashboardHandle`] that must be kept alive for the duration of
/// the daemon. Dropping the handle kills the child process.
pub async fn launch(dashboard_port: u16, daemon_port: u16) -> anyhow::Result<DashboardHandle> {
    let dashboard_dir = find_dashboard_dir()?;
    let runtime = detect_runtime()?;

    tracing::info!(
        dir = %dashboard_dir.display(),
        runtime = %runtime,
        "preparing dashboard"
    );

    generate_env_local(&dashboard_dir, daemon_port, dashboard_port)?;
    install_deps_if_needed(&dashboard_dir, &runtime).await?;
    run_migrations(&dashboard_dir, &runtime).await?;

    let child = spawn_dashboard(&dashboard_dir, &runtime).await?;

    tracing::info!(
        port = dashboard_port,
        "dashboard available at http://localhost:{dashboard_port}"
    );

    Ok(DashboardHandle {
        child: Some(child),
        port: dashboard_port,
    })
}

/// Return the path where the dashboard is expected to live.
pub fn expected_dashboard_path() -> PathBuf {
    // 1. Relative to executable
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let candidate = parent.join("dashboard");
        if candidate.is_dir() {
            return candidate;
        }
        // Also try one level up (e.g. target/release/../dashboard)
        if let Some(grandparent) = parent.parent() {
            let candidate = grandparent.join("dashboard");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }

    // 2. Env var
    if let Ok(path) = std::env::var("SYMPHONY_DASHBOARD_PATH") {
        let p = PathBuf::from(&path);
        if p.is_dir() {
            return p;
        }
    }

    // 3. Default
    dirs_fallback().join("dashboard")
}

/// Check whether the dashboard directory exists.
pub fn dashboard_dir_exists() -> bool {
    expected_dashboard_path().is_dir()
}

/// Check if a dashboard dev server is reachable on the given port.
pub async fn is_running(port: u16) -> bool {
    let url = format!("http://localhost:{port}");
    reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}

/// Kill a dashboard process listening on the given port (best-effort via lsof).
pub async fn stop(port: u16) -> anyhow::Result<()> {
    let output = Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            let pids = String::from_utf8_lossy(&o.stdout);
            for pid in pids.split_whitespace() {
                let _ = Command::new("kill").arg(pid).status().await;
                tracing::info!(pid, "sent SIGTERM to dashboard process");
            }
            Ok(())
        }
        _ => {
            anyhow::bail!("no dashboard process found on port {port}");
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Locate the dashboard directory.
fn find_dashboard_dir() -> anyhow::Result<PathBuf> {
    // 1. Relative to current executable
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let candidate = parent.join("dashboard");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        // Also try two levels up (target/release/../../dashboard)
        if let Some(grandparent) = parent.parent() {
            let candidate = grandparent.join("dashboard");
            if candidate.is_dir() {
                return Ok(candidate);
            }
            // Three levels: target/release/../../dashboard (workspace root)
            if let Some(ggp) = grandparent.parent() {
                let candidate = ggp.join("dashboard");
                if candidate.is_dir() {
                    return Ok(candidate);
                }
            }
        }
    }

    // 2. SYMPHONY_DASHBOARD_PATH env var
    if let Ok(path) = std::env::var("SYMPHONY_DASHBOARD_PATH") {
        let p = PathBuf::from(&path);
        if p.is_dir() {
            return Ok(p);
        }
        tracing::warn!(
            path = %path,
            "SYMPHONY_DASHBOARD_PATH is set but directory does not exist"
        );
    }

    // 3. ~/.symphony/dashboard/
    let fallback = dirs_fallback().join("dashboard");
    if fallback.is_dir() {
        return Ok(fallback);
    }

    anyhow::bail!(
        "dashboard directory not found. Looked in:\n\
         - <exe_dir>/dashboard\n\
         - $SYMPHONY_DASHBOARD_PATH\n\
         - {}\n\n\
         Clone the dashboard into one of these locations or set SYMPHONY_DASHBOARD_PATH.",
        fallback.display()
    );
}

/// Detect the JS runtime (bun preferred, then npx).
fn detect_runtime() -> anyhow::Result<String> {
    if command_exists("bun") {
        return Ok("bun".into());
    }
    if command_exists("npx") {
        return Ok("npx".into());
    }
    anyhow::bail!("no JavaScript runtime found in PATH. Install bun (recommended) or Node.js/npm.");
}

/// Check whether a binary is available in PATH.
fn command_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Generate `.env.local` for the dashboard.
fn generate_env_local(
    dashboard_dir: &Path,
    daemon_port: u16,
    dashboard_port: u16,
) -> anyhow::Result<()> {
    let secret = random_hex_32();
    let content = format!(
        "# Auto-generated by symphony — do not edit\n\
         DATABASE_URL=file:./symphony.db\n\
         AUTH_SECRET={secret}\n\
         SYMPHONY_API_URL=http://localhost:{daemon_port}\n\
         PORT={dashboard_port}\n"
    );

    let env_path = dashboard_dir.join(".env.local");
    std::fs::write(&env_path, content)?;
    tracing::info!(path = %env_path.display(), "wrote .env.local");
    Ok(())
}

/// Generate 32 random hex bytes (64 hex chars).
fn random_hex_32() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Install dependencies if `node_modules/` is missing.
async fn install_deps_if_needed(dashboard_dir: &Path, runtime: &str) -> anyhow::Result<()> {
    let node_modules = dashboard_dir.join("node_modules");
    if node_modules.is_dir() {
        tracing::debug!("node_modules already present, skipping install");
        return Ok(());
    }

    tracing::info!("installing dashboard dependencies");
    let status = Command::new(runtime)
        .arg("install")
        .current_dir(dashboard_dir)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("`{runtime} install` failed with {status}");
    }
    Ok(())
}

/// Run database migrations.
async fn run_migrations(dashboard_dir: &Path, runtime: &str) -> anyhow::Result<()> {
    tracing::info!("running dashboard database migrations");

    for script in &["db:generate", "db:migrate"] {
        let status = Command::new(runtime)
            .args(["run", script])
            .current_dir(dashboard_dir)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await?;

        if !status.success() {
            tracing::warn!(
                script,
                "migration script failed (may be expected on first run)"
            );
        }
    }

    Ok(())
}

/// Spawn the dashboard dev server as a background child process.
async fn spawn_dashboard(dashboard_dir: &Path, runtime: &str) -> anyhow::Result<Child> {
    let mut cmd = Command::new(runtime);
    cmd.args(["run", "dev"])
        .current_dir(dashboard_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;

    // Forward stdout to tracing::info
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!(target: "dashboard", "{}", line);
            }
        });
    }

    // Forward stderr to tracing::warn
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!(target: "dashboard", "{}", line);
            }
        });
    }

    Ok(child)
}

/// Fallback home directory path: `~/.symphony`.
fn dirs_fallback() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join(".symphony")
}
