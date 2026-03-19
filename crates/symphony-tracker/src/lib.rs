// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Issue tracker integration (Spec Section 11).
//!
//! Provides a Linear-compatible tracker client that fetches candidate issues,
//! refreshes issue states, and normalizes payloads into the core domain model.

pub mod github;
pub mod graphql_tool;
pub mod linear;
pub mod markdown;

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
    #[error("github_api_request: {0}")]
    GithubApiRequest(String),
    #[error("github_api_status: {status} {body}")]
    GithubApiStatus { status: u16, body: String },
    #[error("markdown_io_error: {0}")]
    MarkdownIoError(String),
    #[error("markdown_parse_error: {0}")]
    MarkdownParseError(String),
}

/// Create a tracker client from config, dispatching on `config.kind`.
pub fn create_tracker(
    config: &symphony_config::types::TrackerConfig,
) -> Result<Box<dyn TrackerClient>, TrackerError> {
    match config.kind.as_str() {
        "linear" => {
            if config.api_key.is_empty() {
                return Err(TrackerError::MissingApiKey);
            }
            if config.project_slug.is_empty() {
                return Err(TrackerError::MissingProjectSlug);
            }
            Ok(Box::new(linear::LinearClient::new(
                config.endpoint.clone(),
                config.api_key.clone(),
                config.project_slug.clone(),
                config.active_states.clone(),
            )))
        }
        "github" => {
            if config.api_key.is_empty() {
                return Err(TrackerError::MissingApiKey);
            }
            if config.project_slug.is_empty() {
                return Err(TrackerError::MissingProjectSlug);
            }
            let client = github::GithubClient::from_slug(
                config.api_key.clone(),
                &config.project_slug,
                config.active_states.clone(),
            )?;
            Ok(Box::new(client))
        }
        "markdown" => {
            if config.project_slug.is_empty() {
                return Err(TrackerError::MissingProjectSlug);
            }
            let issues_dir = symphony_config::loader::expand_path(&config.project_slug);
            let lago_endpoint = if config.endpoint.is_empty()
                || config.endpoint == "https://api.linear.app/graphql"
            {
                None
            } else {
                Some(config.endpoint.clone())
            };
            Ok(Box::new(markdown::MarkdownClient::with_journal(
                std::path::PathBuf::from(issues_dir),
                config.active_states.clone(),
                lago_endpoint,
            )))
        }
        other => Err(TrackerError::UnsupportedKind(other.to_string())),
    }
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

    /// Transition an issue to a new state. Used for done_state transitions.
    /// Returns Ok(()) on success, or TrackerError on failure.
    async fn set_issue_state(&self, issue_id: &str, state: &str) -> Result<(), TrackerError>;
}
