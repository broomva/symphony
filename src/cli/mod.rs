// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! CLI argument parsing and subcommand routing.
//!
//! Provides the `Cli` struct (clap derive) and `Command` enum for all subcommands.
//! Backward compatible: bare `symphony` or `symphony WORKFLOW.md` starts the daemon.

pub mod client;
pub mod config_cmd;
pub mod control;
pub mod init;
pub mod issues;
pub mod logs;
pub mod output;
pub mod run;
pub mod start;
pub mod status;
pub mod workspaces;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Symphony: orchestrate coding agents for project work.
#[derive(Parser, Debug)]
#[command(name = "symphony", version, about)]
pub struct Cli {
    /// HTTP server port for client commands (env: SYMPHONY_PORT).
    #[arg(long, global = true, env = "SYMPHONY_PORT")]
    pub port: Option<u16>,

    /// Remote daemon host (e.g. symphony.up.railway.app).
    #[arg(long, global = true, env = "SYMPHONY_HOST")]
    pub host: Option<String>,

    /// API bearer token for authenticated access (env: SYMPHONY_API_TOKEN).
    #[arg(long, global = true, env = "SYMPHONY_API_TOKEN")]
    pub token: Option<String>,

    /// Output format.
    #[arg(long, global = true, default_value = "table", value_enum)]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start the Symphony daemon.
    Start(StartArgs),
    /// Graceful shutdown via API.
    Stop,
    /// Query running daemon state.
    Status,
    /// List running + retrying issues.
    Issues,
    /// Detail for one issue.
    Issue(IssueArgs),
    /// Trigger immediate poll.
    Refresh,
    /// List workspace directories.
    Workspaces,
    /// Show/manage a workspace.
    Workspace(WorkspaceArgs),
    /// Validate workflow without starting.
    Validate(ValidateArgs),
    /// Display resolved config.
    Config(ConfigArgs),
    /// Run make smoke equivalent.
    Check,
    /// Full control audit.
    Audit,
    /// Run tests with filtering.
    Test(TestArgs),
    /// Run a single issue one-shot (no daemon loop).
    Run(RunArgs),
    /// Tail daemon log file.
    Logs(LogsArgs),
    /// Initialize a WORKFLOW.md in the current directory.
    Init(InitArgs),
}

#[derive(clap::Args, Debug)]
pub struct RunArgs {
    /// Issue identifier (e.g. STI-123).
    pub identifier: String,

    /// Path to WORKFLOW.md file.
    #[arg(long, default_value = "WORKFLOW.md")]
    pub workflow_path: PathBuf,

    /// Max turns (overrides config).
    #[arg(long)]
    pub turns: Option<u32>,
}

#[derive(clap::Args, Debug)]
pub struct StartArgs {
    /// Path to WORKFLOW.md file.
    #[arg(default_value = "WORKFLOW.md")]
    pub workflow_path: PathBuf,

    /// Log file path (defaults to stderr).
    #[arg(long)]
    pub log_file: Option<PathBuf>,

    /// Max concurrent agents (overrides config).
    #[arg(long, short)]
    pub concurrency: Option<u32>,

    /// Max turns per issue (overrides config).
    #[arg(long)]
    pub turns: Option<u32>,

    /// Run a single poll cycle then exit.
    #[arg(long)]
    pub once: bool,

    /// Only process these specific tickets (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub tickets: Option<Vec<String>>,
}

impl Default for StartArgs {
    fn default() -> Self {
        Self {
            workflow_path: PathBuf::from("WORKFLOW.md"),
            log_file: None,
            concurrency: None,
            turns: None,
            once: false,
            tickets: None,
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct IssueArgs {
    /// Issue identifier (e.g. PROJ-123).
    pub identifier: String,
}

#[derive(clap::Args, Debug)]
pub struct WorkspaceArgs {
    /// Issue identifier for the workspace.
    pub identifier: String,
    /// Remove workspace directory.
    #[arg(long)]
    pub clean: bool,
}

#[derive(clap::Args, Debug)]
pub struct ValidateArgs {
    /// Path to WORKFLOW.md file.
    #[arg(default_value = "WORKFLOW.md")]
    pub workflow_path: PathBuf,
}

#[derive(clap::Args, Debug)]
pub struct ConfigArgs {
    /// Path to WORKFLOW.md file.
    #[arg(default_value = "WORKFLOW.md")]
    pub workflow_path: PathBuf,
}

#[derive(clap::Args, Debug)]
pub struct TestArgs {
    /// Filter by crate name.
    #[arg(long)]
    pub crate_name: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct LogsArgs {
    /// Follow log output (like tail -f).
    #[arg(long, short)]
    pub follow: bool,
    /// Filter by issue identifier.
    #[arg(long)]
    pub id: Option<String>,
    /// Log file path.
    #[arg(default_value = "~/.symphony/symphony.log")]
    pub path: String,
}

#[derive(clap::Args, Debug)]
pub struct InitArgs {
    /// Tracker kind (linear or github).
    #[arg(long, default_value = "linear")]
    pub tracker: String,

    /// Output path for the WORKFLOW.md file.
    #[arg(long, default_value = "WORKFLOW.md")]
    pub output: PathBuf,

    /// Overwrite existing file without asking.
    #[arg(long)]
    pub force: bool,
}

/// Connection options for client commands.
pub struct ConnOpts {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub token: Option<String>,
}

impl ConnOpts {
    pub fn client(&self) -> client::SymphonyClient {
        client::build_client(self.host.as_deref(), self.port, self.token.as_deref())
    }

    /// Display label for error messages.
    pub fn target(&self) -> String {
        match &self.host {
            Some(h) => h.clone(),
            None => format!("localhost:{}", self.port.unwrap_or(client::DEFAULT_PORT)),
        }
    }
}

/// Output format for CLI display.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum OutputFormat {
    Table,
    Json,
}

/// Known subcommand names for backward-compatibility detection.
const SUBCOMMANDS: &[&str] = &[
    "start",
    "stop",
    "status",
    "issues",
    "issue",
    "refresh",
    "workspaces",
    "workspace",
    "validate",
    "config",
    "check",
    "audit",
    "test",
    "run",
    "logs",
    "init",
    "help",
];

/// Parse CLI with backward compatibility for `symphony WORKFLOW.md`.
pub fn parse_cli() -> Cli {
    match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            // Check if first positional arg might be a legacy workflow path
            let args: Vec<String> = std::env::args().collect();
            if let Some(first_pos) = args.get(1)
                && !first_pos.starts_with('-')
                && !SUBCOMMANDS.contains(&first_pos.to_lowercase().as_str())
            {
                // Re-parse with "start" injected before the path
                let mut new_args = vec![args[0].clone(), "start".to_string()];
                new_args.extend_from_slice(&args[1..]);
                if let Ok(cli) = Cli::try_parse_from(new_args) {
                    return cli;
                }
            }
            err.exit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_args_defaults_to_none() {
        let cli = Cli::parse_from(["symphony"]);
        assert!(cli.command.is_none());
        assert!(cli.port.is_none());
        assert_eq!(cli.format, OutputFormat::Table);
    }

    #[test]
    fn parse_start_subcommand() {
        let cli = Cli::parse_from(["symphony", "start"]);
        assert!(matches!(cli.command, Some(Command::Start(_))));
        if let Some(Command::Start(args)) = cli.command {
            assert_eq!(args.workflow_path, PathBuf::from("WORKFLOW.md"));
        }
    }

    #[test]
    fn parse_start_with_path() {
        let cli = Cli::parse_from(["symphony", "start", "/tmp/custom.md"]);
        if let Some(Command::Start(args)) = cli.command {
            assert_eq!(args.workflow_path, PathBuf::from("/tmp/custom.md"));
        }
    }

    #[test]
    fn parse_start_with_port() {
        let cli = Cli::parse_from(["symphony", "--port", "9090", "start"]);
        assert_eq!(cli.port, Some(9090));
        assert!(matches!(cli.command, Some(Command::Start(_))));
    }

    #[test]
    fn parse_stop() {
        let cli = Cli::parse_from(["symphony", "stop"]);
        assert!(matches!(cli.command, Some(Command::Stop)));
    }

    #[test]
    fn parse_status() {
        let cli = Cli::parse_from(["symphony", "status"]);
        assert!(matches!(cli.command, Some(Command::Status)));
    }

    #[test]
    fn parse_issues() {
        let cli = Cli::parse_from(["symphony", "issues"]);
        assert!(matches!(cli.command, Some(Command::Issues)));
    }

    #[test]
    fn parse_issue_with_id() {
        let cli = Cli::parse_from(["symphony", "issue", "PROJ-123"]);
        if let Some(Command::Issue(args)) = cli.command {
            assert_eq!(args.identifier, "PROJ-123");
        }
    }

    #[test]
    fn parse_validate() {
        let cli = Cli::parse_from(["symphony", "validate", "/tmp/wf.md"]);
        if let Some(Command::Validate(args)) = cli.command {
            assert_eq!(args.workflow_path, PathBuf::from("/tmp/wf.md"));
        }
    }

    #[test]
    fn parse_config() {
        let cli = Cli::parse_from(["symphony", "config"]);
        assert!(matches!(cli.command, Some(Command::Config(_))));
    }

    #[test]
    fn parse_check() {
        let cli = Cli::parse_from(["symphony", "check"]);
        assert!(matches!(cli.command, Some(Command::Check)));
    }

    #[test]
    fn parse_audit() {
        let cli = Cli::parse_from(["symphony", "audit"]);
        assert!(matches!(cli.command, Some(Command::Audit)));
    }

    #[test]
    fn parse_test_with_crate() {
        let cli = Cli::parse_from(["symphony", "test", "--crate-name", "symphony-core"]);
        if let Some(Command::Test(args)) = cli.command {
            assert_eq!(args.crate_name, Some("symphony-core".into()));
        }
    }

    #[test]
    fn parse_json_format() {
        let cli = Cli::parse_from(["symphony", "--format", "json", "status"]);
        assert_eq!(cli.format, OutputFormat::Json);
    }

    #[test]
    fn parse_workspace_clean() {
        let cli = Cli::parse_from(["symphony", "workspace", "PROJ-1", "--clean"]);
        if let Some(Command::Workspace(args)) = cli.command {
            assert_eq!(args.identifier, "PROJ-1");
            assert!(args.clean);
        }
    }

    #[test]
    fn parse_logs_follow() {
        let cli = Cli::parse_from(["symphony", "logs", "--follow"]);
        if let Some(Command::Logs(args)) = cli.command {
            assert!(args.follow);
        }
    }

    #[test]
    fn parse_refresh() {
        let cli = Cli::parse_from(["symphony", "refresh"]);
        assert!(matches!(cli.command, Some(Command::Refresh)));
    }

    // S46: backward compat — bare `symphony` starts daemon
    #[test]
    fn backward_compat_bare_symphony() {
        let cli = Cli::parse_from(["symphony"]);
        // None command → treated as Start with defaults
        assert!(cli.command.is_none());
    }
}
