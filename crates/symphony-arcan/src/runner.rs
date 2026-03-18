// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Arcan-based agent runner for Symphony.
//!
//! Replaces CLI subprocess spawning with Arcan HTTP session API calls.

use std::path::Path;

use tracing::info;

use crate::client::{
    ArcanClientConfig, ArcanHttpClient, CreateSessionRequest, PolicyConfig, RunRequest,
};

/// Configuration for the Arcan runtime.
#[derive(Debug, Clone)]
pub struct ArcanRuntimeConfig {
    pub base_url: String,
    pub policy: Option<ArcanPolicyConfig>,
    pub timeout_secs: u64,
}

/// Policy configuration for Arcan sessions.
#[derive(Debug, Clone)]
pub struct ArcanPolicyConfig {
    pub allow_capabilities: Vec<String>,
    pub gate_capabilities: Vec<String>,
}

impl Default for ArcanRuntimeConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3000".to_string(),
            policy: None,
            timeout_secs: 3600,
        }
    }
}

/// Errors from Arcan runner operations.
#[derive(Debug, thiserror::Error)]
pub enum ArcanRunnerError {
    #[error("arcan unavailable: {0}")]
    Unavailable(String),
    #[error("session creation failed: {0}")]
    SessionCreation(String),
    #[error("run failed: {0}")]
    RunFailed(String),
    #[error("client error: {0}")]
    Client(#[from] crate::client::ArcanClientError),
}

/// Agent runner that dispatches work through the Arcan HTTP daemon.
///
/// Drop-in alternative to Symphony's subprocess-based `AgentRunner`.
/// Uses Arcan's session/run API instead of spawning a CLI process.
pub struct ArcanAgentRunner {
    client: ArcanHttpClient,
    config: ArcanRuntimeConfig,
}

impl ArcanAgentRunner {
    pub fn new(config: ArcanRuntimeConfig) -> Self {
        let client_config = ArcanClientConfig {
            base_url: config.base_url.clone(),
            timeout_secs: config.timeout_secs,
        };
        Self {
            client: ArcanHttpClient::new(client_config),
            config,
        }
    }

    /// Run an agent session via Arcan for a given issue.
    ///
    /// Creates an Arcan session, executes a run with the given prompt,
    /// and returns when the run completes.
    pub async fn run_session(
        &self,
        _workspace_path: &Path,
        prompt: &str,
        issue_identifier: &str,
        _issue_title: &str,
        _attempt: Option<u32>,
        _max_turns: u32,
    ) -> Result<ArcanSessionResult, ArcanRunnerError> {
        // Check health first
        let healthy = self.client.health().await.unwrap_or(false);
        if !healthy {
            return Err(ArcanRunnerError::Unavailable(format!(
                "Arcan daemon not reachable at {}",
                self.config.base_url
            )));
        }

        // Create session with policy
        let session_id = format!("symphony-{issue_identifier}");
        let policy = self.config.policy.as_ref().map(|p| PolicyConfig {
            allow_capabilities: p.allow_capabilities.clone(),
            gate_capabilities: p.gate_capabilities.clone(),
            max_tool_runtime_secs: 120,
            max_events_per_turn: 256,
        });

        let create_request = CreateSessionRequest {
            session_id: Some(session_id.clone()),
            owner: Some("symphony".to_string()),
            policy,
        };

        let manifest = self
            .client
            .create_session(&create_request)
            .await
            .map_err(|e| ArcanRunnerError::SessionCreation(e.to_string()))?;

        info!(
            session_id = %manifest.session_id,
            identifier = %issue_identifier,
            "arcan session created"
        );

        // Execute run
        let run_request = RunRequest {
            objective: prompt.to_string(),
            branch: None,
        };

        let run_response = self
            .client
            .run(&manifest.session_id, &run_request)
            .await
            .map_err(|e| ArcanRunnerError::RunFailed(e.to_string()))?;

        info!(
            session_id = %manifest.session_id,
            events = run_response.events_emitted,
            mode = %run_response.mode,
            "arcan run completed"
        );

        Ok(ArcanSessionResult {
            session_id: manifest.session_id,
            events_emitted: run_response.events_emitted,
            last_sequence: run_response.last_sequence,
            mode: run_response.mode,
        })
    }
}

/// Result of an Arcan-based agent session.
#[derive(Debug, Clone)]
pub struct ArcanSessionResult {
    pub session_id: String,
    pub events_emitted: u64,
    pub last_sequence: u64,
    pub mode: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn run_session_full_flow() {
        let server = MockServer::start().await;

        // Mock health check
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        // Mock session creation
        Mock::given(method("POST"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "session_id": "symphony-T-42",
                "owner": "symphony"
            })))
            .mount(&server)
            .await;

        // Mock run execution
        Mock::given(method("POST"))
            .and(path("/sessions/symphony-T-42/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "session_id": "symphony-T-42",
                "mode": "autonomous",
                "events_emitted": 15,
                "last_sequence": 14
            })))
            .mount(&server)
            .await;

        let runner = ArcanAgentRunner::new(ArcanRuntimeConfig {
            base_url: server.uri(),
            timeout_secs: 5,
            policy: None,
        });

        let result = runner
            .run_session(
                Path::new("/tmp/workspace"),
                "Fix the bug in parser.rs",
                "T-42",
                "Fix parser bug",
                None,
                10,
            )
            .await
            .unwrap();

        assert_eq!(result.session_id, "symphony-T-42");
        assert_eq!(result.events_emitted, 15);
        assert_eq!(result.last_sequence, 14);
        assert_eq!(result.mode, "autonomous");
    }

    #[tokio::test]
    async fn run_session_unhealthy_daemon() {
        // Use a port that is not listening
        let runner = ArcanAgentRunner::new(ArcanRuntimeConfig {
            base_url: "http://127.0.0.1:19998".to_string(),
            timeout_secs: 1,
            policy: None,
        });

        let err = runner
            .run_session(
                Path::new("/tmp/workspace"),
                "test prompt",
                "T-1",
                "Test",
                None,
                5,
            )
            .await
            .unwrap_err();

        match err {
            ArcanRunnerError::Unavailable(msg) => {
                assert!(msg.contains("not reachable"));
            }
            other => panic!("expected Unavailable, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn run_session_creation_failure() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .mount(&server)
            .await;

        let runner = ArcanAgentRunner::new(ArcanRuntimeConfig {
            base_url: server.uri(),
            timeout_secs: 5,
            policy: None,
        });

        let err = runner
            .run_session(
                Path::new("/tmp/workspace"),
                "prompt",
                "T-1",
                "Test",
                None,
                5,
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ArcanRunnerError::SessionCreation(_)));
    }

    #[tokio::test]
    async fn run_session_run_failure() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "session_id": "symphony-T-1",
                "owner": "symphony"
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/sessions/symphony-T-1/runs"))
            .respond_with(ResponseTemplate::new(422).set_body_string("bad objective"))
            .mount(&server)
            .await;

        let runner = ArcanAgentRunner::new(ArcanRuntimeConfig {
            base_url: server.uri(),
            timeout_secs: 5,
            policy: None,
        });

        let err = runner
            .run_session(
                Path::new("/tmp/workspace"),
                "prompt",
                "T-1",
                "Test",
                None,
                5,
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ArcanRunnerError::RunFailed(_)));
    }

    #[tokio::test]
    async fn run_session_with_policy() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "session_id": "symphony-T-99",
                "owner": "symphony"
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/sessions/symphony-T-99/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "session_id": "symphony-T-99",
                "mode": "supervised",
                "events_emitted": 3,
                "last_sequence": 2
            })))
            .mount(&server)
            .await;

        let runner = ArcanAgentRunner::new(ArcanRuntimeConfig {
            base_url: server.uri(),
            timeout_secs: 5,
            policy: Some(ArcanPolicyConfig {
                allow_capabilities: vec!["read".into(), "write".into()],
                gate_capabilities: vec!["shell".into()],
            }),
        });

        let result = runner
            .run_session(
                Path::new("/tmp/workspace"),
                "Implement feature X",
                "T-99",
                "Feature X",
                Some(2),
                20,
            )
            .await
            .unwrap();

        assert_eq!(result.session_id, "symphony-T-99");
        assert_eq!(result.mode, "supervised");
    }
}
