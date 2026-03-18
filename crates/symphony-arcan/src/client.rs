// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! HTTP client for the Arcan agent runtime daemon.

use serde::{Deserialize, Serialize};
use tracing::info;

/// Arcan HTTP client configuration.
#[derive(Debug, Clone)]
pub struct ArcanClientConfig {
    pub base_url: String,
    pub timeout_secs: u64,
}

impl Default for ArcanClientConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3000".to_string(),
            timeout_secs: 10,
        }
    }
}

/// HTTP client for the Arcan daemon.
pub struct ArcanHttpClient {
    client: reqwest::Client,
    base_url: String,
}

/// Error type for Arcan client operations.
#[derive(Debug, thiserror::Error)]
pub enum ArcanClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("arcan error: {status} — {message}")]
    ArcanError { status: u16, message: String },
    #[error("session not found: {0}")]
    SessionNotFound(String),
}

// --- Local mirror types (no dependency on Arcan internals) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<PolicyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub allow_capabilities: Vec<String>,
    #[serde(default)]
    pub gate_capabilities: Vec<String>,
    #[serde(default = "default_max_tool_runtime")]
    pub max_tool_runtime_secs: u64,
    #[serde(default = "default_max_events")]
    pub max_events_per_turn: u64,
}

fn default_max_tool_runtime() -> u64 {
    120
}
fn default_max_events() -> u64 {
    256
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionManifest {
    pub session_id: String,
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRequest {
    pub objective: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResponse {
    pub session_id: String,
    pub mode: String,
    pub events_emitted: u64,
    pub last_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResponse {
    pub session_id: String,
    pub mode: String,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveApprovalRequest {
    pub approved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

impl ArcanHttpClient {
    pub fn new(config: ArcanClientConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            base_url: config.base_url,
        }
    }

    /// Create a new session on the Arcan daemon.
    pub async fn create_session(
        &self,
        request: &CreateSessionRequest,
    ) -> Result<SessionManifest, ArcanClientError> {
        let url = format!("{}/sessions", self.base_url);
        let resp = self.client.post(&url).json(request).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ArcanClientError::ArcanError {
                status,
                message: body,
            });
        }
        Ok(resp.json().await?)
    }

    /// Execute a run on a session.
    pub async fn run(
        &self,
        session_id: &str,
        request: &RunRequest,
    ) -> Result<RunResponse, ArcanClientError> {
        let url = format!("{}/sessions/{}/runs", self.base_url, session_id);
        info!(session_id, "executing arcan run");
        let resp = self.client.post(&url).json(request).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ArcanClientError::ArcanError {
                status,
                message: body,
            });
        }
        Ok(resp.json().await?)
    }

    /// Get session state.
    pub async fn get_state(&self, session_id: &str) -> Result<StateResponse, ArcanClientError> {
        let url = format!("{}/sessions/{}/state", self.base_url, session_id);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ArcanClientError::ArcanError {
                status,
                message: body,
            });
        }
        Ok(resp.json().await?)
    }

    /// Resolve an approval request.
    pub async fn resolve_approval(
        &self,
        session_id: &str,
        approval_id: &str,
        approved: bool,
    ) -> Result<(), ArcanClientError> {
        let url = format!(
            "{}/sessions/{}/approvals/{}",
            self.base_url, session_id, approval_id
        );
        let request = ResolveApprovalRequest {
            approved,
            actor: Some("symphony".to_string()),
        };
        let resp = self.client.post(&url).json(&request).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ArcanClientError::ArcanError {
                status,
                message: body,
            });
        }
        Ok(())
    }

    /// Check if the Arcan daemon is healthy.
    pub async fn health(&self) -> Result<bool, ArcanClientError> {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn health_returns_true_when_daemon_is_up() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = ArcanHttpClient::new(ArcanClientConfig {
            base_url: server.uri(),
            timeout_secs: 5,
        });

        assert!(client.health().await.unwrap());
    }

    #[tokio::test]
    async fn health_returns_false_when_daemon_is_down() {
        // Use a port that is (almost certainly) not listening
        let client = ArcanHttpClient::new(ArcanClientConfig {
            base_url: "http://127.0.0.1:19999".to_string(),
            timeout_secs: 1,
        });

        assert!(!client.health().await.unwrap());
    }

    #[tokio::test]
    async fn create_session_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "session_id": "sess-123",
                "owner": "symphony"
            })))
            .mount(&server)
            .await;

        let client = ArcanHttpClient::new(ArcanClientConfig {
            base_url: server.uri(),
            timeout_secs: 5,
        });

        let req = CreateSessionRequest {
            session_id: Some("sess-123".into()),
            owner: Some("symphony".into()),
            policy: None,
        };
        let manifest = client.create_session(&req).await.unwrap();
        assert_eq!(manifest.session_id, "sess-123");
        assert_eq!(manifest.owner, "symphony");
    }

    #[tokio::test]
    async fn create_session_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(409).set_body_string("session already exists"))
            .mount(&server)
            .await;

        let client = ArcanHttpClient::new(ArcanClientConfig {
            base_url: server.uri(),
            timeout_secs: 5,
        });

        let req = CreateSessionRequest {
            session_id: Some("sess-123".into()),
            owner: None,
            policy: None,
        };
        let err = client.create_session(&req).await.unwrap_err();
        match err {
            ArcanClientError::ArcanError { status, message } => {
                assert_eq!(status, 409);
                assert!(message.contains("already exists"));
            }
            other => panic!("expected ArcanError, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn run_session_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sessions/sess-1/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "session_id": "sess-1",
                "mode": "autonomous",
                "events_emitted": 42,
                "last_sequence": 41
            })))
            .mount(&server)
            .await;

        let client = ArcanHttpClient::new(ArcanClientConfig {
            base_url: server.uri(),
            timeout_secs: 5,
        });

        let req = RunRequest {
            objective: "Fix the bug".into(),
            branch: None,
        };
        let resp = client.run("sess-1", &req).await.unwrap();
        assert_eq!(resp.session_id, "sess-1");
        assert_eq!(resp.events_emitted, 42);
        assert_eq!(resp.last_sequence, 41);
        assert_eq!(resp.mode, "autonomous");
    }

    #[tokio::test]
    async fn get_state_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sessions/sess-1/state"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "session_id": "sess-1",
                "mode": "autonomous",
                "version": 5
            })))
            .mount(&server)
            .await;

        let client = ArcanHttpClient::new(ArcanClientConfig {
            base_url: server.uri(),
            timeout_secs: 5,
        });

        let state = client.get_state("sess-1").await.unwrap();
        assert_eq!(state.session_id, "sess-1");
        assert_eq!(state.version, 5);
    }

    #[tokio::test]
    async fn resolve_approval_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sessions/sess-1/approvals/appr-1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = ArcanHttpClient::new(ArcanClientConfig {
            base_url: server.uri(),
            timeout_secs: 5,
        });

        client
            .resolve_approval("sess-1", "appr-1", true)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn run_error_returns_arcan_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sessions/sess-bad/runs"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal server error"))
            .mount(&server)
            .await;

        let client = ArcanHttpClient::new(ArcanClientConfig {
            base_url: server.uri(),
            timeout_secs: 5,
        });

        let req = RunRequest {
            objective: "test".into(),
            branch: None,
        };
        let err = client.run("sess-bad", &req).await.unwrap_err();
        match err {
            ArcanClientError::ArcanError { status, .. } => assert_eq!(status, 500),
            other => panic!("expected ArcanError, got: {other:?}"),
        }
    }
}
