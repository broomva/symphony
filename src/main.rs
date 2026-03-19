// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Symphony - Coding agent orchestration service.
//!
//! A long-running daemon that polls an issue tracker (Linear),
//! creates isolated workspaces per issue, and runs coding agent sessions.
//!
//! Provides a comprehensive CLI for both daemon control and offline operations.

mod cli;

use cli::{Command, StartArgs};

fn main() -> anyhow::Result<()> {
    // Load .env file if present (best-effort, missing file is fine)
    let _ = dotenvy::dotenv();

    let parsed = cli::parse_cli();

    // Resolve the command: None → Start with defaults (backward compat, S46)
    let command = parsed
        .command
        .unwrap_or(Command::Start(StartArgs::default()));

    // Commands that don't need the async runtime
    match &command {
        Command::Validate(_)
        | Command::Config(_)
        | Command::Check
        | Command::Audit
        | Command::Test(_)
        | Command::Logs(_) => {}
        _ => {}
    }

    // Build and run the async runtime
    let conn = cli::ConnOpts {
        host: parsed.host,
        port: parsed.port,
        token: parsed.token,
    };

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(run_command(command, conn, parsed.format));

    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

async fn run_command(
    command: Command,
    conn: cli::ConnOpts,
    format: cli::OutputFormat,
) -> anyhow::Result<()> {
    match command {
        Command::Start(args) => {
            symphony_observability::init_logging();
            cli::start::run_start(args, conn.port).await
        }
        Command::Stop => cli::status::run_stop(&conn).await,
        Command::Status => cli::status::run_status(&conn, format).await,
        Command::Issues => cli::issues::run_issues(&conn, format).await,
        Command::Issue(args) => cli::issues::run_issue(&args.identifier, &conn, format).await,
        Command::Refresh => cli::issues::run_refresh(&conn).await,
        Command::Workspaces => cli::workspaces::run_workspaces(&conn, format).await,
        Command::Workspace(args) => {
            cli::workspaces::run_workspace(
                &args.identifier,
                args.clean,
                &args.workflow_path,
                &conn,
                format,
            )
            .await
        }
        Command::Validate(args) => cli::control::run_validate(&args.workflow_path, format).await,
        Command::Config(args) => cli::config_cmd::run_config(&args.workflow_path, format).await,
        Command::Check => cli::control::run_check().await,
        Command::Audit => cli::control::run_audit().await,
        Command::Test(args) => cli::control::run_test(args.crate_name.as_deref()).await,
        Command::Run(args) => {
            symphony_observability::init_logging();
            cli::run::run_run(args).await
        }
        Command::Logs(args) => cli::logs::run_logs(&args).await,
        Command::Init(args) => {
            cli::init::run_init(&args)?;
            Ok(())
        }
        Command::Doctor => cli::doctor::run_doctor(&conn).await,
        Command::Dashboard(sub) => cli::dashboard::run_dashboard_command(sub).await,
    }
}

#[cfg(test)]
mod tests {
    use super::cli::*;
    use clap::Parser;
    use std::io::Write;
    use std::path::PathBuf;
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

    // S46: backward compat — bare `symphony` works as start
    #[test]
    fn cli_default_no_subcommand() {
        let cli = Cli::parse_from(["symphony"]);
        assert!(cli.command.is_none());
        assert!(cli.port.is_none());
    }

    #[test]
    fn cli_start_explicit() {
        let cli = Cli::parse_from(["symphony", "start"]);
        assert!(matches!(cli.command, Some(Command::Start(_))));
    }

    #[test]
    fn cli_start_with_path() {
        let cli = Cli::parse_from(["symphony", "start", "/tmp/custom.md"]);
        if let Some(Command::Start(args)) = cli.command {
            assert_eq!(args.workflow_path, PathBuf::from("/tmp/custom.md"));
        }
    }

    #[test]
    fn cli_port_flag_global() {
        let f = make_valid_workflow();
        let cli = Cli::parse_from([
            "symphony",
            "--port",
            "8080",
            "start",
            f.path().to_str().unwrap(),
        ]);
        assert_eq!(cli.port, Some(8080));
    }

    #[test]
    fn cli_port_overrides_config() {
        let cli = Cli::parse_from(["symphony", "--port", "8080", "status"]);
        let config_port = Some(3000u16);
        let effective = cli.port.or(config_port);
        assert_eq!(effective, Some(8080));
    }

    #[test]
    fn cli_config_port_used_when_no_flag() {
        let cli = Cli::parse_from(["symphony", "status"]);
        let config_port = Some(3000u16);
        let effective = cli.port.or(config_port);
        assert_eq!(effective, Some(3000));
    }

    #[test]
    fn cli_format_json() {
        let cli = Cli::parse_from(["symphony", "--format", "json", "status"]);
        assert_eq!(cli.format, OutputFormat::Json);
    }

    #[test]
    fn cli_validate_subcommand() {
        let cli = Cli::parse_from(["symphony", "validate", "/tmp/wf.md"]);
        if let Some(Command::Validate(args)) = cli.command {
            assert_eq!(args.workflow_path, PathBuf::from("/tmp/wf.md"));
        }
    }

    #[test]
    fn cli_check_subcommand() {
        let cli = Cli::parse_from(["symphony", "check"]);
        assert!(matches!(cli.command, Some(Command::Check)));
    }

    #[test]
    fn cli_init_subcommand() {
        let cli = Cli::parse_from(["symphony", "init"]);
        assert!(matches!(cli.command, Some(Command::Init(_))));
    }

    #[test]
    fn cli_init_with_tracker() {
        let cli = Cli::parse_from(["symphony", "init", "--tracker", "github"]);
        if let Some(Command::Init(args)) = cli.command {
            assert_eq!(args.tracker, "github");
        }
    }

    #[test]
    fn cli_start_dashboard_defaults() {
        let cli = Cli::parse_from(["symphony", "start"]);
        if let Some(Command::Start(args)) = cli.command {
            assert!(!args.dashboard);
            assert_eq!(args.dashboard_port, 3000);
        }
    }

    #[test]
    fn cli_start_dashboard_flag() {
        let cli = Cli::parse_from([
            "symphony",
            "start",
            "--dashboard",
            "--dashboard-port",
            "4200",
        ]);
        if let Some(Command::Start(args)) = cli.command {
            assert!(args.dashboard);
            assert_eq!(args.dashboard_port, 4200);
        }
    }

    #[test]
    fn cli_dashboard_subcommand() {
        let cli = Cli::parse_from(["symphony", "dashboard", "install"]);
        assert!(matches!(
            cli.command,
            Some(Command::Dashboard(DashboardCommand::Install))
        ));
    }
}
