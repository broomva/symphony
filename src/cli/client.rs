//! HTTP client for communicating with the Symphony daemon.
//!
//! Used by all online commands (status, stop, issues, refresh, etc.).
//! Supports local and remote daemons with optional bearer token auth.

use std::time::Duration;

use symphony_observability::server::StateSummary;

/// HTTP client to the Symphony daemon API.
pub struct SymphonyClient {
    base_url: String,
    token: Option<String>,
    client: reqwest::Client,
}

/// Default daemon port.
pub const DEFAULT_PORT: u16 = 8080;

impl SymphonyClient {
    /// Create a new client targeting the given port on localhost.
    pub fn new(port: u16) -> Self {
        Self::with_url(format!("http://127.0.0.1:{port}"), None)
    }

    /// Create a client targeting a full URL with optional auth token.
    pub fn with_url(base_url: String, token: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("failed to create HTTP client");

        Self {
            base_url,
            token,
            client,
        }
    }

    /// Build a request with optional auth header.
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, &url);
        if let Some(ref token) = self.token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        req
    }

    /// Check if the daemon is running by hitting GET /api/v1/state.
    pub async fn is_running(&self) -> bool {
        self.get_state().await.is_ok()
    }

    /// GET /api/v1/state — system summary.
    pub async fn get_state(&self) -> Result<StateSummary, ClientError> {
        let resp = self
            .request(reqwest::Method::GET, "/api/v1/state")
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 401 {
            return Err(ClientError::Unauthorized);
        }
        if !status.is_success() {
            return Err(ClientError::Http(status.as_u16(), status.to_string()));
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }

    /// GET /api/v1/{identifier} — issue detail.
    pub async fn get_issue(&self, identifier: &str) -> Result<serde_json::Value, ClientError> {
        let resp = self
            .request(reqwest::Method::GET, &format!("/api/v1/{identifier}"))
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 401 {
            return Err(ClientError::Unauthorized);
        }
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
        let resp = self
            .request(reqwest::Method::POST, "/api/v1/refresh")
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        if resp.status().as_u16() == 401 {
            return Err(ClientError::Unauthorized);
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }

    /// POST /api/v1/shutdown — graceful shutdown.
    pub async fn shutdown(&self) -> Result<serde_json::Value, ClientError> {
        let resp = self
            .request(reqwest::Method::POST, "/api/v1/shutdown")
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        if resp.status().as_u16() == 401 {
            return Err(ClientError::Unauthorized);
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }

    /// GET /api/v1/workspaces — list workspaces.
    pub async fn get_workspaces(&self) -> Result<serde_json::Value, ClientError> {
        let resp = self
            .request(reqwest::Method::GET, "/api/v1/workspaces")
            .send()
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 401 {
            return Err(ClientError::Unauthorized);
        }
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
    #[error("unauthorized — set SYMPHONY_API_TOKEN or use --token")]
    Unauthorized,
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

/// Build a SymphonyClient from CLI options.
pub fn build_client(host: Option<&str>, port: Option<u16>, token: Option<&str>) -> SymphonyClient {
    let token = token.map(String::from).or_else(|| {
        std::env::var("SYMPHONY_API_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())
    });

    match host {
        Some(h) => {
            let base_url = if h.starts_with("http://") || h.starts_with("https://") {
                h.trim_end_matches('/').to_string()
            } else {
                format!("https://{}", h.trim_end_matches('/'))
            };
            SymphonyClient::with_url(base_url, token)
        }
        None => {
            let p = port.unwrap_or(DEFAULT_PORT);
            if token.is_some() {
                SymphonyClient::with_url(format!("http://127.0.0.1:{p}"), token)
            } else {
                SymphonyClient::new(p)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_client_local_default() {
        let c = build_client(None, None, None);
        assert_eq!(c.base_url, "http://127.0.0.1:8080");
        assert!(c.token.is_none());
    }

    #[test]
    fn build_client_with_port() {
        let c = build_client(None, Some(9090), None);
        assert_eq!(c.base_url, "http://127.0.0.1:9090");
    }

    #[test]
    fn build_client_with_host() {
        let c = build_client(Some("symphony.up.railway.app"), None, Some("tok"));
        assert_eq!(c.base_url, "https://symphony.up.railway.app");
        assert_eq!(c.token.as_deref(), Some("tok"));
    }

    #[test]
    fn build_client_host_with_scheme() {
        let c = build_client(Some("http://localhost:3000"), None, None);
        assert_eq!(c.base_url, "http://localhost:3000");
    }

    #[test]
    fn client_error_is_connection() {
        let err = ClientError::Connection("refused".into());
        assert!(err.is_connection_error());

        let err = ClientError::Unauthorized;
        assert!(!err.is_connection_error());
    }

    // S47: client commands fail gracefully when daemon unreachable
    #[tokio::test]
    async fn client_fails_gracefully_when_no_daemon() {
        let client = SymphonyClient::new(19999);
        let result = client.get_state().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_connection_error());
    }
}
