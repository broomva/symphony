//! Optional HTTP server extension (Spec Section 13.7).
//!
//! Provides `/` dashboard and `/api/v1/*` JSON endpoints.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::{Json, Router, routing::get};
use serde::Serialize;
use symphony_core::OrchestratorState;
use tokio::sync::Mutex;

/// Shared state for the HTTP server.
#[derive(Clone)]
pub struct AppState {
    pub orchestrator: Arc<Mutex<Option<OrchestratorState>>>,
    pub refresh_tx: Option<tokio::sync::mpsc::Sender<()>>,
}

/// State summary response (Spec Section 13.7.2).
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct StateSummary {
    pub generated_at: String,
    pub counts: Counts,
    pub running: Vec<RunningInfo>,
    pub retrying: Vec<RetryingInfo>,
    pub codex_totals: CodexTotalsInfo,
    pub rate_limits: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct Counts {
    pub running: usize,
    pub retrying: usize,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct RunningInfo {
    pub issue_id: String,
    pub identifier: String,
    pub session_id: Option<String>,
    pub state: String,
    pub started_at: String,
    pub turn_count: u32,
    pub tokens: TokenInfo,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct RetryingInfo {
    pub issue_id: String,
    pub identifier: String,
    pub attempt: u32,
    pub due_at_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct TokenInfo {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct CodexTotalsInfo {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub seconds_running: f64,
}

/// Error envelope (S13.7.2).
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

/// Build the HTTP router (S13.7).
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/api/v1/state", get(get_state))
        .route(
            "/api/v1/refresh",
            axum::routing::post(post_refresh).get(method_not_allowed),
        )
        .route("/api/v1/{identifier}", get(get_issue))
        .with_state(state)
}

/// Dashboard endpoint (S13.7.1).
async fn dashboard(State(state): State<AppState>) -> Html<String> {
    let snapshot = state.orchestrator.lock().await;

    let (running_count, retrying_count, totals) = match snapshot.as_ref() {
        Some(s) => (
            s.running.len(),
            s.retry_attempts.len(),
            &s.codex_totals,
        ),
        None => {
            return Html("<html><body><h1>Symphony Dashboard</h1><p>Initializing...</p></body></html>".into());
        }
    };

    let running_rows: String = snapshot
        .as_ref()
        .map(|s| {
            s.running
                .values()
                .map(|r| {
                    format!(
                        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        r.identifier,
                        r.issue.state,
                        r.session_id.as_deref().unwrap_or("-"),
                        r.turn_count
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    Html(format!(
        r#"<html>
<head><title>Symphony Dashboard</title>
<style>body {{ font-family: system-ui; max-width: 800px; margin: 40px auto; padding: 0 20px; }}
table {{ border-collapse: collapse; width: 100%; }} th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
th {{ background-color: #f4f4f4; }} .stat {{ display: inline-block; margin: 10px 20px 10px 0; }}</style>
</head>
<body>
<h1>Symphony Dashboard</h1>
<div><span class="stat"><b>Running:</b> {running_count}</span>
<span class="stat"><b>Retrying:</b> {retrying_count}</span>
<span class="stat"><b>Total tokens:</b> {total}</span>
<span class="stat"><b>Runtime:</b> {runtime:.1}s</span></div>
<h2>Active Sessions</h2>
<table><tr><th>Identifier</th><th>State</th><th>Session</th><th>Turns</th></tr>
{running_rows}
</table>
<p><em>Generated at {time}</em></p>
</body></html>"#,
        total = totals.total_tokens,
        runtime = totals.seconds_running,
        time = chrono::Utc::now().to_rfc3339()
    ))
}

/// GET /api/v1/state — system summary (S13.7.2).
async fn get_state(State(state): State<AppState>) -> Json<StateSummary> {
    let snapshot = state.orchestrator.lock().await;
    Json(build_summary(snapshot.as_ref()))
}

/// GET /api/v1/{identifier} — issue-specific detail (S13.7.2).
async fn get_issue(
    State(state): State<AppState>,
    Path(identifier): Path<String>,
) -> impl IntoResponse {
    let snapshot = state.orchestrator.lock().await;

    if let Some(s) = snapshot.as_ref() {
        // Search in running
        if let Some(entry) = s
            .running
            .values()
            .find(|r| r.identifier == identifier)
        {
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "identifier": entry.identifier,
                    "state": entry.issue.state,
                    "session_id": entry.session_id,
                    "started_at": entry.started_at.to_rfc3339(),
                    "turn_count": entry.turn_count,
                    "tokens": {
                        "input_tokens": entry.codex_input_tokens,
                        "output_tokens": entry.codex_output_tokens,
                        "total_tokens": entry.codex_total_tokens
                    }
                })),
            );
        }

        // Search in retry queue
        if let Some(entry) = s
            .retry_attempts
            .values()
            .find(|r| r.identifier == identifier)
        {
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "identifier": entry.identifier,
                    "status": "retrying",
                    "attempt": entry.attempt,
                    "due_at_ms": entry.due_at_ms,
                    "error": entry.error
                })),
            );
        }
    }

    // 404 with error envelope (S13.7.2)
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": {
                "code": "not_found",
                "message": format!("issue '{identifier}' not found")
            }
        })),
    )
}

/// POST /api/v1/refresh — trigger immediate poll (S13.7.2).
async fn post_refresh(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    // Trigger immediate poll if channel available
    let coalesced = if let Some(tx) = &state.refresh_tx {
        tx.try_send(()).is_err() // err = already queued (coalesced)
    } else {
        false
    };

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "queued": true,
            "coalesced": coalesced,
            "requested_at": chrono::Utc::now().to_rfc3339(),
            "operations": ["poll", "reconcile"]
        })),
    )
}

/// 405 Method Not Allowed handler (S13.7.2).
async fn method_not_allowed() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(serde_json::json!({
            "error": {
                "code": "method_not_allowed",
                "message": "use POST for this endpoint"
            }
        })),
    )
}

fn build_summary(snapshot: Option<&OrchestratorState>) -> StateSummary {
    match snapshot {
        Some(s) => {
            let running: Vec<RunningInfo> = s
                .running
                .values()
                .map(|r| RunningInfo {
                    issue_id: r.issue.id.clone(),
                    identifier: r.identifier.clone(),
                    session_id: r.session_id.clone(),
                    state: r.issue.state.clone(),
                    started_at: r.started_at.to_rfc3339(),
                    turn_count: r.turn_count,
                    tokens: TokenInfo {
                        input_tokens: r.codex_input_tokens,
                        output_tokens: r.codex_output_tokens,
                        total_tokens: r.codex_total_tokens,
                    },
                })
                .collect();

            let retrying: Vec<RetryingInfo> = s
                .retry_attempts
                .values()
                .map(|r| RetryingInfo {
                    issue_id: r.issue_id.clone(),
                    identifier: r.identifier.clone(),
                    attempt: r.attempt,
                    due_at_ms: r.due_at_ms,
                    error: r.error.clone(),
                })
                .collect();

            StateSummary {
                generated_at: chrono::Utc::now().to_rfc3339(),
                counts: Counts {
                    running: running.len(),
                    retrying: retrying.len(),
                },
                running,
                retrying,
                codex_totals: CodexTotalsInfo {
                    input_tokens: s.codex_totals.input_tokens,
                    output_tokens: s.codex_totals.output_tokens,
                    total_tokens: s.codex_totals.total_tokens,
                    seconds_running: s.codex_totals.seconds_running,
                },
                rate_limits: s.codex_rate_limits.clone(),
            }
        }
        None => StateSummary {
            generated_at: chrono::Utc::now().to_rfc3339(),
            counts: Counts {
                running: 0,
                retrying: 0,
            },
            running: vec![],
            retrying: vec![],
            codex_totals: CodexTotalsInfo {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                seconds_running: 0.0,
            },
            rate_limits: None,
        },
    }
}

/// Start the HTTP server on the given port (S13.7).
/// Binds to loopback 127.0.0.1 by default.
/// Port 0 = ephemeral.
pub async fn start_server(port: u16) -> anyhow::Result<()> {
    let state = AppState {
        orchestrator: Arc::new(Mutex::new(None)),
        refresh_tx: None,
    };
    start_server_with_state(port, state).await
}

/// Start the HTTP server with shared state.
pub async fn start_server_with_state(port: u16, state: AppState) -> anyhow::Result<()> {
    let app = build_router(state);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!(%addr, "starting HTTP server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn make_app_state() -> AppState {
        AppState {
            orchestrator: Arc::new(Mutex::new(Some(OrchestratorState::new(30000, 10)))),
            refresh_tx: None,
        }
    }

    #[tokio::test]
    async fn dashboard_returns_html() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_state_returns_json() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/api/v1/state")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let summary: StateSummary = serde_json::from_slice(&body).unwrap();
        assert_eq!(summary.counts.running, 0);
        assert_eq!(summary.counts.retrying, 0);
    }

    #[tokio::test]
    async fn get_unknown_issue_returns_404() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/api/v1/UNKNOWN-999")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("error").is_some());
        assert_eq!(json["error"]["code"], "not_found");
    }

    #[tokio::test]
    async fn post_refresh_returns_202() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/refresh")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn get_refresh_returns_405() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/api/v1/refresh")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
