//! Agent runner (Spec Section 10).
//!
//! Creates workspace, builds prompt, starts app-server session,
//! forwards events to orchestrator.

use std::path::Path;
use std::process::Stdio;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

use symphony_config::types::CodexConfig;

use crate::protocol::{AgentEvent, ProtocolMessage, TokenUsage, TurnOutcome};

/// Linear auth config for the optional `linear_graphql` tool (S10.5).
#[derive(Debug, Clone)]
pub struct LinearToolConfig {
    pub endpoint: String,
    pub api_key: String,
}

/// Agent runner wrapping workspace + prompt + app-server client.
pub struct AgentRunner {
    codex_config: CodexConfig,
    linear_tool: Option<LinearToolConfig>,
}

/// Errors from agent runner operations (S10.6).
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
    #[error("response_error: {0}")]
    ResponseError(String),
    #[error("turn_failed: {0}")]
    TurnFailed(String),
    #[error("turn_cancelled")]
    TurnCancelled,
    #[error("turn_input_required")]
    TurnInputRequired,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Session state tracked during a single agent run.
pub struct AgentSession {
    pub thread_id: String,
    pub turn_id: String,
    pub session_id: String,
    pub turn_count: u32,
    pub token_usage: TokenUsage,
}

/// Callback type for agent events.
pub type EventCallback = Box<dyn Fn(AgentEvent) + Send + Sync>;

impl AgentRunner {
    pub fn new(codex_config: CodexConfig) -> Self {
        Self {
            codex_config,
            linear_tool: None,
        }
    }

    /// Create an agent runner with the optional `linear_graphql` tool (S10.5).
    pub fn with_linear_tool(codex_config: CodexConfig, linear_tool: LinearToolConfig) -> Self {
        Self {
            codex_config,
            linear_tool: Some(linear_tool),
        }
    }

    /// Launch a coding agent subprocess (S10.1).
    fn spawn_process(&self, workspace_path: &Path) -> Result<Child, AgentError> {
        // Validate workspace path (S9.5 Invariant 1)
        if !workspace_path.is_dir() {
            return Err(AgentError::InvalidWorkspaceCwd(
                workspace_path.display().to_string(),
            ));
        }

        let child = tokio::process::Command::new("bash")
            .args(["-lc", &self.codex_config.command])
            .current_dir(workspace_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AgentError::CodexNotFound(self.codex_config.command.clone())
                } else {
                    AgentError::Io(e)
                }
            })?;

        Ok(child)
    }

    /// Send a JSON-RPC message to the process stdin.
    async fn send_message(stdin: &mut ChildStdin, msg: &ProtocolMessage) -> Result<(), AgentError> {
        let json = serde_json::to_string(msg).map_err(|e| AgentError::Io(e.into()))?;
        stdin
            .write_all(json.as_bytes())
            .await
            .map_err(AgentError::Io)?;
        stdin.write_all(b"\n").await.map_err(AgentError::Io)?;
        stdin.flush().await.map_err(AgentError::Io)?;
        Ok(())
    }

    /// Read a single line-delimited JSON message from stdout (S10.3).
    async fn read_message(
        reader: &mut BufReader<ChildStdout>,
        timeout_ms: u64,
    ) -> Result<ProtocolMessage, AgentError> {
        let mut line = String::new();

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            reader.read_line(&mut line),
        )
        .await;

        match result {
            Ok(Ok(0)) => Err(AgentError::ProcessExit),
            Ok(Ok(_)) => {
                let msg: ProtocolMessage = serde_json::from_str(line.trim())
                    .map_err(|e| AgentError::ResponseError(e.to_string()))?;
                Ok(msg)
            }
            Ok(Err(e)) => Err(AgentError::Io(e)),
            Err(_) => Err(AgentError::ResponseTimeout),
        }
    }

    /// Perform the startup handshake (S10.2).
    ///
    /// Sends: initialize → wait for response → initialized → thread/start →
    ///        wait for response → turn/start → wait for response.
    #[allow(clippy::too_many_arguments)]
    async fn handshake(
        stdin: &mut ChildStdin,
        reader: &mut BufReader<ChildStdout>,
        workspace_path: &Path,
        prompt: &str,
        identifier: &str,
        title: &str,
        read_timeout: u64,
        codex_config: &CodexConfig,
        advertised_tools: Option<&Vec<Value>>,
    ) -> Result<AgentSession, AgentError> {
        let cwd_str = workspace_path.display().to_string();

        // 1. initialize
        let init_msg = ProtocolMessage::request(
            1,
            "initialize",
            serde_json::json!({
                "clientInfo": { "name": "symphony", "version": "1.0" },
                "capabilities": {}
            }),
        );
        Self::send_message(stdin, &init_msg).await?;

        // Wait for initialize response
        let _init_resp = Self::read_message(reader, read_timeout).await?;

        // 2. initialized notification
        let initialized = ProtocolMessage::notification("initialized", serde_json::json!({}));
        Self::send_message(stdin, &initialized).await?;

        // 3. thread/start (S10.5: advertise tools if configured)
        let approval_policy = codex_config
            .approval_policy
            .as_deref()
            .unwrap_or("auto-edit");
        let mut thread_params = serde_json::json!({
            "approvalPolicy": approval_policy,
            "sandbox": codex_config.thread_sandbox.as_deref().unwrap_or("none"),
            "cwd": cwd_str,
        });
        // Advertise optional client-side tools (S10.5)
        if let Some(tools) = advertised_tools {
            thread_params
                .as_object_mut()
                .unwrap()
                .insert("tools".into(), Value::Array(tools.clone()));
        }
        let thread_start = ProtocolMessage::request(2, "thread/start", thread_params);
        Self::send_message(stdin, &thread_start).await?;

        let thread_resp = Self::read_message(reader, read_timeout).await?;
        let thread_id = thread_resp
            .result
            .as_ref()
            .and_then(|r| r.get("threadId"))
            .and_then(|t| t.as_str())
            .unwrap_or("unknown-thread")
            .to_string();

        // 4. turn/start (first turn with full prompt)
        let sandbox_policy = codex_config
            .turn_sandbox_policy
            .as_deref()
            .unwrap_or("none");
        let turn_title = format!("{identifier}: {title}");
        let turn_start = ProtocolMessage::request(
            3,
            "turn/start",
            serde_json::json!({
                "threadId": thread_id,
                "input": prompt,
                "cwd": cwd_str,
                "title": turn_title,
                "approvalPolicy": approval_policy,
                "sandboxPolicy": sandbox_policy,
            }),
        );
        Self::send_message(stdin, &turn_start).await?;

        let turn_resp = Self::read_message(reader, read_timeout).await?;
        let turn_id = turn_resp
            .result
            .as_ref()
            .and_then(|r| r.get("turnId"))
            .and_then(|t| t.as_str())
            .unwrap_or("unknown-turn")
            .to_string();

        let session_id = format!("{thread_id}-{turn_id}");

        Ok(AgentSession {
            thread_id,
            turn_id,
            session_id,
            turn_count: 1,
            token_usage: TokenUsage::default(),
        })
    }

    /// Stream turn events until completion (S10.3).
    async fn stream_turn(
        stdin: &mut ChildStdin,
        reader: &mut BufReader<ChildStdout>,
        session: &mut AgentSession,
        turn_timeout_ms: u64,
        linear_tool: &Option<LinearToolConfig>,
        on_event: &EventCallback,
    ) -> TurnOutcome {
        let deadline =
            tokio::time::Instant::now() + std::time::Duration::from_millis(turn_timeout_ms);

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return TurnOutcome::Timeout;
            }

            let mut line = String::new();
            let result = tokio::time::timeout(remaining, reader.read_line(&mut line)).await;

            match result {
                Ok(Ok(0)) => return TurnOutcome::ProcessExit,
                Ok(Ok(_)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<Value>(trimmed) {
                        Ok(msg) => {
                            // Check for tool call requests (S10.5)
                            if let Some("tool/call") = msg.get("method").and_then(|m| m.as_str()) {
                                let tool_call_id = msg
                                    .get("id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let params = msg.get("params").cloned().unwrap_or(Value::Null);
                                let tool_name =
                                    params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                let input = params.get("input").cloned().unwrap_or(Value::Null);

                                let response = Self::handle_tool_call(
                                    &tool_call_id,
                                    tool_name,
                                    &input,
                                    linear_tool,
                                    on_event,
                                )
                                .await;

                                // Send tool response back to agent
                                let response_msg = ProtocolMessage {
                                    id: Some(Value::String(tool_call_id)),
                                    method: None,
                                    params: None,
                                    result: response.get("result").cloned(),
                                    error: None,
                                };
                                let _ = Self::send_message(stdin, &response_msg).await;
                                continue;
                            }

                            let outcome = Self::handle_message(&msg, session, on_event);
                            if let Some(outcome) = outcome {
                                return outcome;
                            }
                        }
                        Err(_) => {
                            on_event(AgentEvent::Malformed {
                                raw: trimmed.to_string(),
                            });
                        }
                    }
                }
                Ok(Err(_)) => return TurnOutcome::ProcessExit,
                Err(_) => return TurnOutcome::Timeout,
            }
        }
    }

    /// Handle a tool call from the agent (S10.5).
    ///
    /// Returns a response message if the tool was handled, or None for unsupported tools.
    async fn handle_tool_call(
        tool_call_id: &str,
        tool_name: &str,
        input: &Value,
        linear_tool: &Option<LinearToolConfig>,
        on_event: &EventCallback,
    ) -> Value {
        if tool_name == "linear_graphql" {
            if let Some(config) = linear_tool {
                match symphony_tracker::graphql_tool::validate_input(input) {
                    Ok((query, variables)) => {
                        let result = symphony_tracker::graphql_tool::execute_graphql_tool(
                            &config.endpoint,
                            &config.api_key,
                            &query,
                            variables,
                        )
                        .await;
                        serde_json::json!({
                            "id": tool_call_id,
                            "result": result
                        })
                    }
                    Err(err) => {
                        serde_json::json!({
                            "id": tool_call_id,
                            "result": { "success": false, "error": err }
                        })
                    }
                }
            } else {
                // Linear tool not configured (missing auth)
                serde_json::json!({
                    "id": tool_call_id,
                    "result": { "success": false, "error": "missing_linear_auth" }
                })
            }
        } else {
            // Unsupported tool call → failure result, continue session (S10.5)
            on_event(AgentEvent::UnsupportedToolCall {
                id: tool_call_id.to_string(),
                name: tool_name.to_string(),
            });
            serde_json::json!({
                "id": tool_call_id,
                "result": { "success": false, "error": "unsupported_tool_call" }
            })
        }
    }

    /// Handle a single protocol message during turn streaming.
    fn handle_message(
        msg: &Value,
        session: &mut AgentSession,
        on_event: &EventCallback,
    ) -> Option<TurnOutcome> {
        let method = msg.get("method").and_then(|m| m.as_str());

        match method {
            Some("turn/completed") => {
                // Extract token usage if present (S13.5)
                Self::extract_usage(msg, session);
                on_event(AgentEvent::TurnCompleted {
                    usage: Some(session.token_usage.clone()),
                });
                Some(TurnOutcome::Completed)
            }
            Some("turn/failed") => {
                let error = msg
                    .get("params")
                    .and_then(|p| p.get("error"))
                    .and_then(|e| e.as_str())
                    .unwrap_or("unknown error")
                    .to_string();
                Self::extract_usage(msg, session);
                on_event(AgentEvent::TurnFailed {
                    error: error.clone(),
                    usage: Some(session.token_usage.clone()),
                });
                Some(TurnOutcome::Failed(error))
            }
            Some("turn/cancelled") => {
                Self::extract_usage(msg, session);
                on_event(AgentEvent::TurnCancelled {
                    usage: Some(session.token_usage.clone()),
                });
                Some(TurnOutcome::Cancelled)
            }
            Some("turn/inputRequired") => {
                // User input required → hard failure (S10.5)
                on_event(AgentEvent::TurnInputRequired);
                Some(TurnOutcome::InputRequired)
            }
            Some("approval/request") => {
                // Auto-approve (S10.5: auto-approve all)
                let id = msg
                    .get("params")
                    .and_then(|p| p.get("id"))
                    .and_then(|i| i.as_str())
                    .unwrap_or("")
                    .to_string();
                on_event(AgentEvent::ApprovalAutoApproved { id });
                None // continue streaming
            }
            Some("thread/tokenUsage/updated") => {
                // Absolute token totals (S13.5)
                Self::extract_absolute_usage(msg, session);
                None
            }
            Some(m) => {
                on_event(AgentEvent::OtherMessage {
                    method: Some(m.to_string()),
                    payload: msg.clone(),
                });
                None
            }
            None => {
                // No method — might be a response to our request
                None
            }
        }
    }

    /// Extract token usage from a message (S13.5).
    fn extract_usage(msg: &Value, session: &mut AgentSession) {
        if let Some(usage) = msg.get("params").and_then(|p| p.get("usage")) {
            if let Some(input) = usage.get("inputTokens").and_then(|v| v.as_u64()) {
                session.token_usage.input_tokens = input;
            }
            if let Some(output) = usage.get("outputTokens").and_then(|v| v.as_u64()) {
                session.token_usage.output_tokens = output;
            }
            if let Some(total) = usage.get("totalTokens").and_then(|v| v.as_u64()) {
                session.token_usage.total_tokens = total;
            }
        }
    }

    /// Extract absolute token usage from thread/tokenUsage/updated (S13.5).
    fn extract_absolute_usage(msg: &Value, session: &mut AgentSession) {
        if let Some(params) = msg.get("params") {
            if let Some(input) = params.get("inputTokens").and_then(|v| v.as_u64()) {
                session.token_usage.input_tokens = input;
            }
            if let Some(output) = params.get("outputTokens").and_then(|v| v.as_u64()) {
                session.token_usage.output_tokens = output;
            }
            if let Some(total) = params.get("totalTokens").and_then(|v| v.as_u64()) {
                session.token_usage.total_tokens = total;
            }
        }
    }

    /// Launch a simple (non-JSON-RPC) agent session.
    ///
    /// Pipes the prompt as stdin to the command and waits for completion.
    /// Used for CLI agents like `claude` that don't speak JSON-RPC.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_simple_session(
        &self,
        workspace_path: &Path,
        prompt: &str,
        issue_identifier: &str,
        _issue_title: &str,
        _attempt: Option<u32>,
        _max_turns: u32,
        on_event: &EventCallback,
    ) -> Result<AgentSession, AgentError> {
        if !workspace_path.is_dir() {
            return Err(AgentError::InvalidWorkspaceCwd(
                workspace_path.display().to_string(),
            ));
        }

        // Build command: append `-p` with the prompt for claude CLI
        let full_command = format!("{} -p {}", self.codex_config.command, shell_escape(prompt));

        let mut child = tokio::process::Command::new("bash")
            .args(["-lc", &full_command])
            .current_dir(workspace_path)
            .env_remove("CLAUDECODE")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AgentError::CodexNotFound(self.codex_config.command.clone())
                } else {
                    AgentError::Io(e)
                }
            })?;

        let pid = child.id().map(|p| p.to_string());
        let session_id = format!("simple-{issue_identifier}");

        on_event(AgentEvent::SessionStarted {
            session_id: session_id.clone(),
            thread_id: session_id.clone(),
            turn_id: "turn-1".into(),
            pid: pid.clone(),
        });

        // Log stdout and stderr in background
        if let Some(stdout) = child.stdout.take() {
            let ident = issue_identifier.to_string();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    tracing::info!(identifier = %ident, stdout = line.trim(), "agent output");
                    line.clear();
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let ident = issue_identifier.to_string();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    tracing::warn!(identifier = %ident, stderr = line.trim(), "agent stderr");
                    line.clear();
                }
            });
        }

        // Wait for completion with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(self.codex_config.turn_timeout_ms),
            child.wait(),
        )
        .await;

        match result {
            Ok(Ok(status)) => {
                if status.success() {
                    on_event(AgentEvent::TurnCompleted { usage: None });
                    Ok(AgentSession {
                        thread_id: session_id.clone(),
                        turn_id: "turn-1".into(),
                        session_id,
                        turn_count: 1,
                        token_usage: TokenUsage::default(),
                    })
                } else {
                    let msg = format!("agent exited with status: {status}");
                    on_event(AgentEvent::TurnFailed {
                        error: msg.clone(),
                        usage: None,
                    });
                    Err(AgentError::TurnFailed(msg))
                }
            }
            Ok(Err(e)) => Err(AgentError::Io(e)),
            Err(_) => {
                let _ = child.kill().await;
                Err(AgentError::TurnTimeout)
            }
        }
    }

    /// Launch a coding agent session in the given workspace (S10.1-10.6).
    ///
    /// Handles: subprocess launch → handshake → turn streaming → multi-turn loop.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_session(
        &self,
        workspace_path: &Path,
        prompt: &str,
        issue_identifier: &str,
        issue_title: &str,
        _attempt: Option<u32>,
        max_turns: u32,
        on_event: &EventCallback,
    ) -> Result<AgentSession, AgentError> {
        let mut child = self.spawn_process(workspace_path)?;

        let pid = child.id().map(|p| p.to_string());
        let mut stdin = child.stdin.take().ok_or(AgentError::ProcessExit)?;
        let stdout = child.stdout.take().ok_or(AgentError::ProcessExit)?;
        let mut reader = BufReader::new(stdout);

        // Log stderr in background (S10.3: stderr is diagnostics, not protocol)
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut stderr_reader = BufReader::new(stderr);
                let mut line = String::new();
                while stderr_reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    tracing::debug!(stderr = line.trim(), "agent stderr");
                    line.clear();
                }
            });
        }

        // Build advertised tools list (S10.5)
        let tools = self
            .linear_tool
            .as_ref()
            .map(|_| vec![symphony_tracker::graphql_tool::tool_spec()]);

        // Perform handshake
        let mut session = Self::handshake(
            &mut stdin,
            &mut reader,
            workspace_path,
            prompt,
            issue_identifier,
            issue_title,
            self.codex_config.read_timeout_ms,
            &self.codex_config,
            tools.as_ref(),
        )
        .await
        .inspect_err(|e| {
            on_event(AgentEvent::StartupFailed {
                error: e.to_string(),
            });
        })?;

        on_event(AgentEvent::SessionStarted {
            session_id: session.session_id.clone(),
            thread_id: session.thread_id.clone(),
            turn_id: session.turn_id.clone(),
            pid,
        });

        // Stream first turn
        let mut outcome = Self::stream_turn(
            &mut stdin,
            &mut reader,
            &mut session,
            self.codex_config.turn_timeout_ms,
            &self.linear_tool,
            on_event,
        )
        .await;

        // Multi-turn continuation loop (S10.3, S7.1)
        while outcome == TurnOutcome::Completed && session.turn_count < max_turns {
            // Issue another turn on same thread (S10.3)
            session.turn_count += 1;

            // Continuation turn with guidance only, not re-sending original prompt (S7.1)
            let continuation_prompt = "Continue working on the issue. Check if there's more to do.";
            let next_turn = ProtocolMessage::request(
                (session.turn_count + 2) as u64,
                "turn/start",
                serde_json::json!({
                    "threadId": session.thread_id,
                    "input": continuation_prompt,
                    "cwd": workspace_path.display().to_string(),
                }),
            );

            if Self::send_message(&mut stdin, &next_turn).await.is_err() {
                break;
            }

            let turn_resp =
                Self::read_message(&mut reader, self.codex_config.read_timeout_ms).await;
            if let Ok(resp) = turn_resp {
                if let Some(new_turn_id) = resp
                    .result
                    .as_ref()
                    .and_then(|r| r.get("turnId"))
                    .and_then(|t| t.as_str())
                {
                    session.turn_id = new_turn_id.to_string();
                    session.session_id = format!("{}-{}", session.thread_id, session.turn_id);
                }
            } else {
                break;
            }

            outcome = Self::stream_turn(
                &mut stdin,
                &mut reader,
                &mut session,
                self.codex_config.turn_timeout_ms,
                &self.linear_tool,
                on_event,
            )
            .await;
        }

        // Cleanup: kill process (S10.3: stop subprocess only when worker run ends)
        let _ = child.kill().await;

        // Map final outcome to result
        match outcome {
            TurnOutcome::Completed => Ok(session),
            TurnOutcome::Failed(msg) => Err(AgentError::TurnFailed(msg)),
            TurnOutcome::Cancelled => Err(AgentError::TurnCancelled),
            TurnOutcome::InputRequired => Err(AgentError::TurnInputRequired),
            TurnOutcome::Timeout => Err(AgentError::TurnTimeout),
            TurnOutcome::ProcessExit => Err(AgentError::ProcessExit),
        }
    }
}

/// Shell-escape a string for safe embedding in a command.
fn shell_escape(s: &str) -> String {
    // Use single quotes and escape any embedded single quotes
    let escaped = s.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_error_display() {
        assert_eq!(
            AgentError::CodexNotFound("codex".into()).to_string(),
            "codex_not_found: command 'codex' not found"
        );
        assert_eq!(AgentError::ResponseTimeout.to_string(), "response_timeout");
        assert_eq!(AgentError::TurnTimeout.to_string(), "turn_timeout");
        assert_eq!(
            AgentError::ProcessExit.to_string(),
            "port_exit: process exited unexpectedly"
        );
        assert_eq!(AgentError::TurnCancelled.to_string(), "turn_cancelled");
        assert_eq!(
            AgentError::TurnInputRequired.to_string(),
            "turn_input_required"
        );
    }

    #[test]
    fn invalid_workspace_path() {
        let runner = AgentRunner::new(CodexConfig::default());
        let err = runner.spawn_process(Path::new("/nonexistent/workspace"));
        assert!(err.is_err());
        assert!(matches!(
            err.unwrap_err(),
            AgentError::InvalidWorkspaceCwd(_)
        ));
    }

    #[test]
    fn session_id_composition() {
        let session = AgentSession {
            thread_id: "thread-abc".into(),
            turn_id: "turn-123".into(),
            session_id: "thread-abc-turn-123".into(),
            turn_count: 1,
            token_usage: TokenUsage::default(),
        };
        assert_eq!(
            session.session_id,
            format!("{}-{}", session.thread_id, session.turn_id)
        );
    }

    #[test]
    fn extract_usage_from_message() {
        let msg = serde_json::json!({
            "method": "turn/completed",
            "params": {
                "usage": {
                    "inputTokens": 100,
                    "outputTokens": 50,
                    "totalTokens": 150
                }
            }
        });
        let mut session = AgentSession {
            thread_id: "t".into(),
            turn_id: "u".into(),
            session_id: "t-u".into(),
            turn_count: 1,
            token_usage: TokenUsage::default(),
        };
        AgentRunner::extract_usage(&msg, &mut session);
        assert_eq!(session.token_usage.input_tokens, 100);
        assert_eq!(session.token_usage.output_tokens, 50);
        assert_eq!(session.token_usage.total_tokens, 150);
    }

    #[test]
    fn extract_absolute_usage() {
        let msg = serde_json::json!({
            "method": "thread/tokenUsage/updated",
            "params": {
                "inputTokens": 500,
                "outputTokens": 200,
                "totalTokens": 700
            }
        });
        let mut session = AgentSession {
            thread_id: "t".into(),
            turn_id: "u".into(),
            session_id: "t-u".into(),
            turn_count: 1,
            token_usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
            },
        };
        AgentRunner::extract_absolute_usage(&msg, &mut session);
        assert_eq!(session.token_usage.input_tokens, 500);
        assert_eq!(session.token_usage.output_tokens, 200);
        assert_eq!(session.token_usage.total_tokens, 700);
    }

    #[test]
    fn handle_turn_completed() {
        let msg = serde_json::json!({ "method": "turn/completed", "params": {} });
        let mut session = AgentSession {
            thread_id: "t".into(),
            turn_id: "u".into(),
            session_id: "t-u".into(),
            turn_count: 1,
            token_usage: TokenUsage::default(),
        };
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let cb: EventCallback = Box::new(move |e| {
            events_clone.lock().unwrap().push(e);
        });

        let outcome = AgentRunner::handle_message(&msg, &mut session, &cb);
        assert_eq!(outcome, Some(TurnOutcome::Completed));
    }

    #[test]
    fn handle_turn_failed() {
        let msg = serde_json::json!({
            "method": "turn/failed",
            "params": { "error": "something broke" }
        });
        let mut session = AgentSession {
            thread_id: "t".into(),
            turn_id: "u".into(),
            session_id: "t-u".into(),
            turn_count: 1,
            token_usage: TokenUsage::default(),
        };
        let cb: EventCallback = Box::new(|_| {});
        let outcome = AgentRunner::handle_message(&msg, &mut session, &cb);
        assert_eq!(outcome, Some(TurnOutcome::Failed("something broke".into())));
    }

    #[test]
    fn handle_input_required() {
        let msg = serde_json::json!({ "method": "turn/inputRequired", "params": {} });
        let mut session = AgentSession {
            thread_id: "t".into(),
            turn_id: "u".into(),
            session_id: "t-u".into(),
            turn_count: 1,
            token_usage: TokenUsage::default(),
        };
        let cb: EventCallback = Box::new(|_| {});
        let outcome = AgentRunner::handle_message(&msg, &mut session, &cb);
        assert_eq!(outcome, Some(TurnOutcome::InputRequired));
    }

    #[test]
    fn handle_approval_auto_approved() {
        let msg = serde_json::json!({
            "method": "approval/request",
            "params": { "id": "approval-1" }
        });
        let mut session = AgentSession {
            thread_id: "t".into(),
            turn_id: "u".into(),
            session_id: "t-u".into(),
            turn_count: 1,
            token_usage: TokenUsage::default(),
        };
        let cb: EventCallback = Box::new(|_| {});
        let outcome = AgentRunner::handle_message(&msg, &mut session, &cb);
        // Auto-approve continues session
        assert_eq!(outcome, None);
    }
}
