// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Optional HTTP server extension (Spec Section 13.7).
//!
//! Provides `/` dashboard, `/api/v1/*` JSON endpoints, and `/metrics` Prometheus endpoint.

use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::{Json, Router, routing::get};
use serde::Serialize;
use symphony_core::OrchestratorState;
use tokio::sync::Mutex;

/// Shared state for the HTTP server.
#[derive(Clone)]
pub struct AppState {
    pub orchestrator: Arc<Mutex<Option<OrchestratorState>>>,
    pub refresh_tx: Option<tokio::sync::mpsc::Sender<()>>,
    pub shutdown_tx: Option<Arc<tokio::sync::watch::Sender<bool>>>,
    /// Optional bearer token for API authentication.
    /// When set, all `/api/v1/*` endpoints require `Authorization: Bearer <token>`.
    /// Health endpoints (`/healthz`, `/readyz`) and dashboard (`/`) remain open.
    pub api_token: Option<String>,
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
    // API routes — protected by optional bearer token auth
    let api_routes = Router::new()
        .route("/api/v1/state", get(get_state))
        .route("/api/v1/workspaces", get(get_workspaces))
        .route(
            "/api/v1/refresh",
            axum::routing::post(post_refresh).get(method_not_allowed),
        )
        .route(
            "/api/v1/shutdown",
            axum::routing::post(post_shutdown).get(method_not_allowed),
        )
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/{identifier}", get(get_issue))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Public routes — no auth required
    Router::new()
        .route("/", get(dashboard))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(get_prometheus_metrics))
        .merge(api_routes)
        .with_state(state)
}

/// Bearer token auth middleware. Only enforced when `api_token` is configured.
async fn auth_middleware(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if let Some(expected) = &state.api_token {
        let auth_header = request
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok());

        match auth_header {
            Some(header) if header.starts_with("Bearer ") => {
                let token = &header[7..];
                if token != expected.as_str() {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({
                            "error": {
                                "code": "unauthorized",
                                "message": "invalid bearer token"
                            }
                        })),
                    )
                        .into_response();
                }
            }
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": {
                            "code": "unauthorized",
                            "message": "missing Authorization: Bearer <token> header"
                        }
                    })),
                )
                    .into_response();
            }
        }
    }

    next.run(request).await
}

/// Dashboard endpoint (S13.7.1).
async fn dashboard(State(state): State<AppState>) -> Html<String> {
    let snapshot = state.orchestrator.lock().await;

    let (running_count, retrying_count, totals, retrying_rows, poll_ms, max_conc) =
        match snapshot.as_ref() {
            Some(s) => {
                let retrying: String = s
                    .retry_attempts
                    .values()
                    .map(|r| {
                        format!(
                            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                            r.identifier,
                            r.attempt,
                            r.due_at_ms,
                            r.error.as_deref().unwrap_or("-")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (
                    s.running.len(),
                    s.retry_attempts.len(),
                    &s.codex_totals,
                    retrying,
                    s.poll_interval_ms,
                    s.max_concurrent_agents,
                )
            }
            None => {
                return Html(
                    "<html><body><h1>Symphony Dashboard</h1><p>Initializing...</p></body></html>"
                        .into(),
                );
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
<meta http-equiv="refresh" content="5">
<style>body {{ font-family: system-ui; max-width: 800px; margin: 40px auto; padding: 0 20px; }}
table {{ border-collapse: collapse; width: 100%; margin-bottom: 20px; }} th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
th {{ background-color: #f4f4f4; }} .stat {{ display: inline-block; margin: 10px 20px 10px 0; }}</style>
</head>
<body>
<h1>Symphony Dashboard</h1>
<div><span class="stat"><b>Running:</b> {running_count}</span>
<span class="stat"><b>Retrying:</b> {retrying_count}</span>
<span class="stat"><b>Tokens:</b> {input_tokens} in / {output_tokens} out / {total_tokens} total</span>
<span class="stat"><b>Runtime:</b> {runtime:.1}s</span></div>
<div><span class="stat"><b>Poll interval:</b> {poll_ms}ms</span>
<span class="stat"><b>Max concurrent:</b> {max_conc}</span></div>
<h2>Active Sessions</h2>
<table><tr><th>Identifier</th><th>State</th><th>Session</th><th>Turns</th></tr>
{running_rows}
</table>
<h2>Retrying Issues</h2>
<table><tr><th>Identifier</th><th>Attempt</th><th>Due At</th><th>Error</th></tr>
{retrying_rows}
</table>
<p><em>Generated at {time}</em></p>
</body></html>"#,
        input_tokens = totals.input_tokens,
        output_tokens = totals.output_tokens,
        total_tokens = totals.total_tokens,
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
        if let Some(entry) = s.running.values().find(|r| r.identifier == identifier) {
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

/// POST /api/v1/shutdown — graceful shutdown (S45).
async fn post_shutdown(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    if let Some(tx) = &state.shutdown_tx {
        let _ = tx.send(true);
        (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "shutdown": true,
                "requested_at": chrono::Utc::now().to_rfc3339()
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "code": "shutdown_unavailable",
                    "message": "shutdown channel not configured"
                }
            })),
        )
    }
}

/// GET /api/v1/workspaces — list workspace directories.
async fn get_workspaces(State(state): State<AppState>) -> Json<serde_json::Value> {
    let snapshot = state.orchestrator.lock().await;
    let mut entries = Vec::new();

    if let Some(s) = snapshot.as_ref() {
        for entry in s.running.values() {
            entries.push(serde_json::json!({
                "name": entry.identifier,
                "status": "running",
            }));
        }
        for entry in s.retry_attempts.values() {
            entries.push(serde_json::json!({
                "name": entry.identifier,
                "status": "retrying",
            }));
        }
    }

    Json(serde_json::Value::Array(entries))
}

/// GET /api/v1/metrics — usage metrics for metering scrape.
async fn get_metrics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let snapshot = state.orchestrator.lock().await;
    match snapshot.as_ref() {
        Some(s) => {
            // Calculate active session elapsed time
            let now = chrono::Utc::now();
            let active_seconds: f64 = s
                .running
                .values()
                .map(|e| now.signed_duration_since(e.started_at).num_seconds() as f64)
                .sum();

            Json(serde_json::json!({
                "timestamp": now.to_rfc3339(),
                "totals": {
                    "input_tokens": s.codex_totals.input_tokens,
                    "output_tokens": s.codex_totals.output_tokens,
                    "total_tokens": s.codex_totals.total_tokens,
                    "seconds_running": s.codex_totals.seconds_running + active_seconds,
                },
                "current": {
                    "running_sessions": s.running.len(),
                    "retrying_sessions": s.retry_attempts.len(),
                    "claimed_issues": s.claimed.len(),
                },
                "config": {
                    "poll_interval_ms": s.poll_interval_ms,
                    "max_concurrent_agents": s.max_concurrent_agents,
                }
            }))
        }
        None => Json(serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "totals": { "input_tokens": 0, "output_tokens": 0, "total_tokens": 0, "seconds_running": 0.0 },
            "current": { "running_sessions": 0, "retrying_sessions": 0, "claimed_issues": 0 },
            "config": { "poll_interval_ms": 0, "max_concurrent_agents": 0 }
        })),
    }
}

/// GET /metrics — Prometheus/OpenMetrics text format (S56 extension).
///
/// Exposed without auth so Prometheus can scrape without bearer tokens.
/// For authenticated JSON metrics, use `/api/v1/metrics`.
async fn get_prometheus_metrics(State(state): State<AppState>) -> Response {
    let snapshot = state.orchestrator.lock().await;
    let (input, output, total, seconds, running, retrying, claimed, completed, poll_ms, max_conc) =
        match snapshot.as_ref() {
            Some(s) => {
                let now = chrono::Utc::now();
                let active_seconds: f64 = s
                    .running
                    .values()
                    .map(|e| now.signed_duration_since(e.started_at).num_seconds() as f64)
                    .sum();
                (
                    s.codex_totals.input_tokens,
                    s.codex_totals.output_tokens,
                    s.codex_totals.total_tokens,
                    s.codex_totals.seconds_running + active_seconds,
                    s.running.len(),
                    s.retry_attempts.len(),
                    s.claimed.len(),
                    s.completed.len(),
                    s.poll_interval_ms,
                    s.max_concurrent_agents,
                )
            }
            None => (0, 0, 0, 0.0, 0, 0, 0, 0, 0, 0),
        };

    use std::fmt::Write;
    let mut body = String::with_capacity(2048);

    // Token counters
    writeln!(
        body,
        "# HELP symphony_tokens_input_total Total input tokens consumed."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_tokens_input_total counter").unwrap();
    writeln!(body, "symphony_tokens_input_total {input}").unwrap();

    writeln!(
        body,
        "# HELP symphony_tokens_output_total Total output tokens consumed."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_tokens_output_total counter").unwrap();
    writeln!(body, "symphony_tokens_output_total {output}").unwrap();

    writeln!(
        body,
        "# HELP symphony_tokens_total Total tokens consumed (input + output)."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_tokens_total counter").unwrap();
    writeln!(body, "symphony_tokens_total {total}").unwrap();

    // Runtime
    writeln!(
        body,
        "# HELP symphony_agent_seconds_total Total agent runtime in seconds."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_agent_seconds_total counter").unwrap();
    writeln!(body, "symphony_agent_seconds_total {seconds:.3}").unwrap();

    // Session gauges
    writeln!(
        body,
        "# HELP symphony_sessions_running Current running agent sessions."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_sessions_running gauge").unwrap();
    writeln!(body, "symphony_sessions_running {running}").unwrap();

    writeln!(
        body,
        "# HELP symphony_sessions_retrying Current sessions in retry queue."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_sessions_retrying gauge").unwrap();
    writeln!(body, "symphony_sessions_retrying {retrying}").unwrap();

    writeln!(
        body,
        "# HELP symphony_issues_claimed Total issues currently claimed."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_issues_claimed gauge").unwrap();
    writeln!(body, "symphony_issues_claimed {claimed}").unwrap();

    writeln!(
        body,
        "# HELP symphony_issues_completed Total issues completed."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_issues_completed counter").unwrap();
    writeln!(body, "symphony_issues_completed {completed}").unwrap();

    // Config info
    writeln!(
        body,
        "# HELP symphony_config_poll_interval_ms Configured poll interval in milliseconds."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_config_poll_interval_ms gauge").unwrap();
    writeln!(body, "symphony_config_poll_interval_ms {poll_ms}").unwrap();

    writeln!(
        body,
        "# HELP symphony_config_max_concurrent_agents Configured max concurrent agent count."
    )
    .unwrap();
    writeln!(body, "# TYPE symphony_config_max_concurrent_agents gauge").unwrap();
    writeln!(body, "symphony_config_max_concurrent_agents {max_conc}").unwrap();

    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
        .into_response()
}

/// GET /healthz — liveness probe (always 200).
async fn healthz() -> StatusCode {
    StatusCode::OK
}

/// GET /readyz — readiness probe (200 when orchestrator initialized, 503 otherwise).
async fn readyz(State(state): State<AppState>) -> StatusCode {
    if state.orchestrator.lock().await.is_some() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
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
        shutdown_tx: None,
        api_token: None,
    };
    start_server_with_state(port, state, None).await
}

/// Start the HTTP server with shared state and optional graceful shutdown.
pub async fn start_server_with_state(
    port: u16,
    state: AppState,
    shutdown_rx: Option<tokio::sync::watch::Receiver<bool>>,
) -> anyhow::Result<()> {
    let app = build_router(state);
    // Bind 0.0.0.0 when SYMPHONY_BIND=0.0.0.0 or when PORT env is set (Railway/cloud)
    let bind_addr: [u8; 4] = if std::env::var("SYMPHONY_BIND").as_deref() == Ok("0.0.0.0")
        || std::env::var("PORT").is_ok()
    {
        [0, 0, 0, 0]
    } else {
        [127, 0, 0, 1]
    };
    let addr = std::net::SocketAddr::from((bind_addr, port));
    tracing::info!(%addr, "starting HTTP server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    if let Some(mut rx) = shutdown_rx {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.wait_for(|v| *v).await;
            })
            .await?;
    } else {
        axum::serve(listener, app).await?;
    }
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
            shutdown_tx: None,
            api_token: None,
        }
    }

    #[tokio::test]
    async fn dashboard_returns_html() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
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

        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
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

        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
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
    async fn healthz_returns_200() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/healthz")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readyz_returns_200_when_initialized() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/readyz")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readyz_returns_503_when_not_initialized() {
        let state = AppState {
            orchestrator: Arc::new(Mutex::new(None)),
            refresh_tx: None,
            shutdown_tx: None,
            api_token: None,
        };
        let app = build_router(state);
        let req = Request::builder()
            .uri("/readyz")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
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

    #[tokio::test]
    async fn post_shutdown_returns_202() {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
        let state = AppState {
            orchestrator: Arc::new(Mutex::new(Some(OrchestratorState::new(30000, 10)))),
            refresh_tx: None,
            shutdown_tx: Some(Arc::new(shutdown_tx)),
            api_token: None,
        };
        let app = build_router(state);
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/shutdown")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);

        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["shutdown"], true);
    }

    #[tokio::test]
    async fn post_shutdown_without_channel_returns_503() {
        let state = make_app_state(); // no shutdown_tx
        let app = build_router(state);
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/shutdown")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn get_workspaces_returns_array() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/api/v1/workspaces")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.is_array());
    }

    #[tokio::test]
    async fn auth_rejects_missing_token() {
        let state = AppState {
            orchestrator: Arc::new(Mutex::new(Some(OrchestratorState::new(30000, 10)))),
            refresh_tx: None,
            shutdown_tx: None,
            api_token: Some("secret-token".into()),
        };
        let app = build_router(state);
        let req = Request::builder()
            .uri("/api/v1/state")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_rejects_wrong_token() {
        let state = AppState {
            orchestrator: Arc::new(Mutex::new(Some(OrchestratorState::new(30000, 10)))),
            refresh_tx: None,
            shutdown_tx: None,
            api_token: Some("secret-token".into()),
        };
        let app = build_router(state);
        let req = Request::builder()
            .uri("/api/v1/state")
            .header("Authorization", "Bearer wrong-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_accepts_correct_token() {
        let state = AppState {
            orchestrator: Arc::new(Mutex::new(Some(OrchestratorState::new(30000, 10)))),
            refresh_tx: None,
            shutdown_tx: None,
            api_token: Some("secret-token".into()),
        };
        let app = build_router(state);
        let req = Request::builder()
            .uri("/api/v1/state")
            .header("Authorization", "Bearer secret-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_endpoints_bypass_auth() {
        let state = AppState {
            orchestrator: Arc::new(Mutex::new(Some(OrchestratorState::new(30000, 10)))),
            refresh_tx: None,
            shutdown_tx: None,
            api_token: Some("secret-token".into()),
        };
        let app = build_router(state);
        // healthz should work without token
        let req = Request::builder()
            .uri("/healthz")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn prometheus_metrics_returns_text() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let ct = resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(ct.contains("text/plain"));

        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("symphony_tokens_total"));
        assert!(text.contains("# TYPE symphony_sessions_running gauge"));
        assert!(text.contains("symphony_issues_completed"));
    }

    #[tokio::test]
    async fn prometheus_metrics_bypasses_auth() {
        let state = AppState {
            orchestrator: Arc::new(Mutex::new(Some(OrchestratorState::new(30000, 10)))),
            refresh_tx: None,
            shutdown_tx: None,
            api_token: Some("secret-token".into()),
        };
        let app = build_router(state);
        // /metrics should work without token
        let req = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn dashboard_contains_auto_refresh() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains(r#"meta http-equiv="refresh" content="5""#));
    }

    #[tokio::test]
    async fn dashboard_contains_retrying_table() {
        let state = make_app_state();
        let app = build_router(state);
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("Retrying Issues"));
        assert!(text.contains("<th>Attempt</th>"));
    }
}
