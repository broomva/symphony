//! Agent runner (Spec Section 10.7).
//!
//! Creates workspace, builds prompt, starts app-server session,
//! forwards events to orchestrator.

use std::path::Path;

use symphony_config::types::CodexConfig;

/// Agent runner wrapping workspace + prompt + app-server client.
#[allow(dead_code)]
pub struct AgentRunner {
    codex_config: CodexConfig,
}

/// Errors from agent runner operations.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("codex_not_found: command '{0}' not found")]
    CodexNotFound(String),
    #[error("invalid_workspace_cwd: {0}")]
    InvalidWorkspaceCwd(String),
    #[error("response_timeout")]
    ResponseTimeout,
    #[error("turn_timeout")]
    TurnTimeout,
    #[error("port_exit: process exited unexpectedly")]
    ProcessExit,
    #[error("turn_failed: {0}")]
    TurnFailed(String),
    #[error("turn_cancelled")]
    TurnCancelled,
    #[error("turn_input_required")]
    TurnInputRequired,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl AgentRunner {
    pub fn new(codex_config: CodexConfig) -> Self {
        Self { codex_config }
    }

    /// Launch a coding agent session in the given workspace.
    ///
    /// This is a stub — full implementation will:
    /// 1. Spawn `bash -lc <codex.command>` with workspace as cwd
    /// 2. Send initialize, initialized, thread/start, turn/start
    /// 3. Stream turn events and forward to orchestrator
    /// 4. Handle continuation turns
    pub async fn run_session(
        &self,
        _workspace_path: &Path,
        _prompt: &str,
        _issue_identifier: &str,
        _issue_title: &str,
        _attempt: Option<u32>,
    ) -> Result<(), AgentError> {
        // TODO: Implement full app-server protocol
        tracing::warn!("agent runner: stub implementation");
        Ok(())
    }
}
