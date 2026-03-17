//! HTTP client for communicating with the Symphony daemon.
//!
//! Used by all online commands (status, stop, issues, refresh, etc.).

use std::time::Duration;

use symphony_observability::server::StateSummary;

/// HTTP client to the Symphony daemon API.
pub struct SymphonyClient {
    base_url: String,
    client: reqwest::Client,
}

/// Default daemon port.
pub const DEFAULT_PORT: u16 = 8080;

impl SymphonyClient {
    /// Create a new client targeting the given port.
    pub fn new(port: u16) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("failed to create HTTP client");

        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            client,
        }
    }

    /// Check if the daemon is running by hitting GET /api/v1/state.
    pub async fn is_running(&self) -> bool {
        self.get_state().await.is_ok()
    }

    /// GET /api/v1/state — system summary.
    pub async fn get_state(&self) -> Result<StateSummary, ClientError> {
        let url = format!("{}/api/v1/state", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(ClientError::Http(status.as_u16(), status.to_string()));
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }

    /// GET /api/v1/{identifier} — issue detail.
    pub async fn get_issue(
        &self,
        identifier: &str,
    ) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}/api/v1/{}", self.base_url, identifier);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 404 {
            return Err(ClientError::NotFound(identifier.to_string()));
        }
        if !status.is_success() {
            return Err(ClientError::Http(status.as_u16(), status.to_string()));
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }

    /// POST /api/v1/refresh — trigger immediate poll.
    pub async fn refresh(&self) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}/api/v1/refresh", self.base_url);
        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        resp.json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }

    /// POST /api/v1/shutdown — graceful shutdown.
    pub async fn shutdown(&self) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}/api/v1/shutdown", self.base_url);
        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        resp.json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }

    /// GET /api/v1/workspaces — list workspaces.
    pub async fn get_workspaces(&self) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}/api/v1/workspaces", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(ClientError::Http(status.as_u16(), status.to_string()));
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }
}

/// Errors from client operations.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("connection failed (is daemon running?): {0}")]
    Connection(String),
    #[error("HTTP {0}: {1}")]
    Http(u16, String),
    #[error("issue not found: {0}")]
    NotFound(String),
    #[error("response parse error: {0}")]
    Parse(String),
}

impl ClientError {
    /// Whether this error indicates the daemon is not running.
    pub fn is_connection_error(&self) -> bool {
        matches!(self, ClientError::Connection(_))
    }
}

/// Resolve the effective port from CLI arg or default.
pub fn resolve_port(port: Option<u16>) -> u16 {
    port.unwrap_or(DEFAULT_PORT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_port_uses_default() {
        assert_eq!(resolve_port(None), DEFAULT_PORT);
    }

    #[test]
    fn resolve_port_uses_explicit() {
        assert_eq!(resolve_port(Some(9090)), 9090);
    }

    #[test]
    fn client_error_is_connection() {
        let err = ClientError::Connection("refused".into());
        assert!(err.is_connection_error());

        let err = ClientError::Http(500, "server error".into());
        assert!(!err.is_connection_error());
    }

    // S47: client commands fail gracefully when daemon unreachable
    #[tokio::test]
    async fn client_fails_gracefully_when_no_daemon() {
        // Use a port that's almost certainly not listening
        let client = SymphonyClient::new(19999);
        let result = client.get_state().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_connection_error());
    }
}
