//! Linear issue tracker adapter (Spec Sections 11.2, 11.3).

use async_trait::async_trait;
use symphony_core::Issue;

use crate::{TrackerClient, TrackerError};

/// Linear GraphQL client.
#[allow(dead_code)]
pub struct LinearClient {
    endpoint: String,
    api_key: String,
    project_slug: String,
    active_states: Vec<String>,
    http: reqwest::Client,
}

impl LinearClient {
    pub fn new(
        endpoint: String,
        api_key: String,
        project_slug: String,
        active_states: Vec<String>,
    ) -> Self {
        Self {
            endpoint,
            api_key,
            project_slug,
            active_states,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(30_000))
                .build()
                .expect("failed to build HTTP client"),
        }
    }
}

#[async_trait]
impl TrackerClient for LinearClient {
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>, TrackerError> {
        // TODO: Implement Linear GraphQL query for candidate issues
        // Query filters: project slugId, active states, pagination
        tracing::warn!("fetch_candidate_issues: stub implementation");
        Ok(vec![])
    }

    async fn fetch_issues_by_states(&self, states: &[String]) -> Result<Vec<Issue>, TrackerError> {
        if states.is_empty() {
            return Ok(vec![]);
        }
        // TODO: Implement Linear GraphQL query for issues by state
        tracing::warn!("fetch_issues_by_states: stub implementation");
        Ok(vec![])
    }

    async fn fetch_issue_states_by_ids(
        &self,
        issue_ids: &[String],
    ) -> Result<Vec<Issue>, TrackerError> {
        if issue_ids.is_empty() {
            return Ok(vec![]);
        }
        // TODO: Implement Linear GraphQL query for issue states by ID
        tracing::warn!("fetch_issue_states_by_ids: stub implementation");
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_states_returns_empty() {
        let client = LinearClient::new(
            "https://api.linear.app/graphql".into(),
            "test-key".into(),
            "test-proj".into(),
            vec!["Todo".into()],
        );
        let result = client.fetch_issues_by_states(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn empty_ids_returns_empty() {
        let client = LinearClient::new(
            "https://api.linear.app/graphql".into(),
            "test-key".into(),
            "test-proj".into(),
            vec!["Todo".into()],
        );
        let result = client.fetch_issue_states_by_ids(&[]).await.unwrap();
        assert!(result.is_empty());
    }
}
