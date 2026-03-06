//! Observability layer (Spec Section 13).
//!
//! Structured logging, optional HTTP server with dashboard and JSON API.

pub mod server;

/// Initialize structured logging with tracing.
pub fn init_logging() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .json()
        .init();
}
