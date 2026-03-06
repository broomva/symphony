//! Optional HTTP server extension (Spec Section 13.7).
//!
//! Provides `/` dashboard and `/api/v1/*` JSON endpoints.

use axum::{Json, Router, routing::get};
use serde::Serialize;

/// Shared state for the HTTP server.
pub struct AppState {
    // TODO: Add reference to orchestrator for snapshots
}

/// State summary response (Spec Section 13.7.2).
#[derive(Debug, Serialize)]
pub struct StateSummary {
    pub generated_at: String,
    pub counts: Counts,
    pub running: Vec<serde_json::Value>,
    pub retrying: Vec<serde_json::Value>,
    pub codex_totals: serde_json::Value,
    pub rate_limits: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct Counts {
    pub running: usize,
    pub retrying: usize,
}

/// Build the HTTP router.
pub fn build_router() -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/api/v1/state", get(get_state))
        .route("/api/v1/refresh", axum::routing::post(post_refresh))
}

async fn dashboard() -> axum::response::Html<String> {
    axum::response::Html(
        "<html><head><title>Symphony</title></head><body><h1>Symphony Dashboard</h1><p>Status: Running</p></body></html>".into(),
    )
}

async fn get_state() -> Json<StateSummary> {
    // TODO: Pull from orchestrator snapshot
    Json(StateSummary {
        generated_at: chrono::Utc::now().to_rfc3339(),
        counts: Counts {
            running: 0,
            retrying: 0,
        },
        running: vec![],
        retrying: vec![],
        codex_totals: serde_json::json!({
            "input_tokens": 0,
            "output_tokens": 0,
            "total_tokens": 0,
            "seconds_running": 0.0
        }),
        rate_limits: None,
    })
}

async fn post_refresh() -> (axum::http::StatusCode, Json<serde_json::Value>) {
    // TODO: Trigger immediate poll
    (
        axum::http::StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "queued": true,
            "coalesced": false,
            "requested_at": chrono::Utc::now().to_rfc3339(),
            "operations": ["poll", "reconcile"]
        })),
    )
}

/// Start the HTTP server on the given port.
pub async fn start_server(port: u16) -> anyhow::Result<()> {
    let app = build_router();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!(%addr, "starting HTTP server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
