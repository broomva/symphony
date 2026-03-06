//! Issue tracker integration (Spec Section 11).
//!
//! Provides a Linear-compatible tracker client that fetches candidate issues,
//! refreshes issue states, and normalizes payloads into the core domain model.

pub mod graphql_tool;
pub mod linear;

use async_trait::async_trait;
use symphony_core::Issue;

/// Errors from tracker operations (Spec Section 11.4).
#[derive(Debug, thiserror::Error)]
pub enum TrackerError {
    #[error("unsupported_tracker_kind: {0}")]
    UnsupportedKind(String),
    #[error("missing_tracker_api_key")]
    MissingApiKey,
    #[error("missing_tracker_project_slug")]
    MissingProjectSlug,
    #[error("linear_api_request: {0}")]
    ApiRequest(String),
    #[error("linear_api_status: {status} {body}")]
    ApiStatus { status: u16, body: String },
    #[error("linear_graphql_errors: {0}")]
    GraphqlErrors(String),
    #[error("linear_unknown_payload: {0}")]
    UnknownPayload(String),
    #[error("linear_missing_end_cursor")]
    MissingEndCursor,
}

/// Trait for issue tracker adapters (Spec Section 11.1).
#[async_trait]
pub trait TrackerClient: Send + Sync {
    /// Fetch issues in configured active states for the project.
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>, TrackerError>;

    /// Fetch issues in the given states (used for startup terminal cleanup).
    async fn fetch_issues_by_states(&self, states: &[String]) -> Result<Vec<Issue>, TrackerError>;

    /// Fetch current states for specific issue IDs (reconciliation).
    async fn fetch_issue_states_by_ids(
        &self,
        issue_ids: &[String],
    ) -> Result<Vec<Issue>, TrackerError>;
}
