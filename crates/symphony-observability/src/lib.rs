//! Observability layer (Spec Section 13).
//!
//! Structured logging, optional HTTP server with dashboard and JSON API.

pub mod server;

/// Initialize structured logging with tracing (S13.1-13.2).
///
/// - JSON format for machine parsing
/// - EnvFilter for log level control (defaults to "info")
/// - Includes target information for source identification
/// - Sink failure does not crash (S13.2) — tracing handles this gracefully
pub fn init_logging() {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .json()
        .init();
}
