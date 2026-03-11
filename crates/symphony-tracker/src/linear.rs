//! Linear issue tracker adapter (Spec Sections 11.2, 11.3, 11.4).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use symphony_core::{BlockerRef, Issue};

use crate::{TrackerClient, TrackerError};

const PAGE_SIZE: u32 = 50;

/// Linear GraphQL client.
pub struct LinearClient {
    endpoint: String,
    api_key: String,
    project_slug: String,
    #[allow(dead_code)]
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

    /// Get the configured endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Get the configured API key.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Execute a GraphQL query against the Linear API.
    pub async fn graphql_query(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<Value, TrackerError> {
        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });

        let response = self
            .http
            .post(&self.endpoint)
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| TrackerError::ApiRequest(e.to_string()))?;

        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".into());
            return Err(TrackerError::ApiStatus {
                status,
                body: body_text,
            });
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| TrackerError::UnknownPayload(e.to_string()))?;

        // Check for GraphQL errors
        if let Some(errors) = json.get("errors")
            && let Some(arr) = errors.as_array()
            && !arr.is_empty()
        {
            return Err(TrackerError::GraphqlErrors(errors.to_string()));
        }

        json.get("data")
            .cloned()
            .ok_or_else(|| TrackerError::UnknownPayload("missing 'data' in response".into()))
    }

    /// Fetch issues with pagination for a given GraphQL query.
    async fn fetch_paginated_issues(
        &self,
        query: &str,
        build_variables: impl Fn(Option<&str>) -> Value,
        data_path: &[&str],
    ) -> Result<Vec<Issue>, TrackerError> {
        let mut all_issues = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let variables = build_variables(cursor.as_deref());
            let data = self.graphql_query(query, variables).await?;

            // Navigate data_path to find the issues connection
            let mut node = &data;
            for &key in data_path {
                node = node.get(key).ok_or_else(|| {
                    TrackerError::UnknownPayload(format!("missing key '{key}' in response"))
                })?;
            }

            // Extract nodes
            let nodes = node
                .get("nodes")
                .and_then(|n| n.as_array())
                .ok_or_else(|| {
                    TrackerError::UnknownPayload("missing 'nodes' array in response".into())
                })?;

            for node_val in nodes {
                if let Some(issue) = normalize_issue(node_val) {
                    all_issues.push(issue);
                }
            }

            // Check pagination
            let page_info = node.get("pageInfo");
            let has_next = page_info
                .and_then(|p| p.get("hasNextPage"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if !has_next {
                break;
            }

            let end_cursor = page_info
                .and_then(|p| p.get("endCursor"))
                .and_then(|v| v.as_str());

            match end_cursor {
                Some(c) => cursor = Some(c.to_string()),
                None => return Err(TrackerError::MissingEndCursor),
            }
        }

        Ok(all_issues)
    }
}

/// GraphQL query for fetching candidate issues (S11.2).
const CANDIDATE_ISSUES_QUERY: &str = r#"
query CandidateIssues($projectSlug: String!, $first: Int!, $after: String) {
  issues(
    filter: {
      project: { slugId: { eq: $projectSlug } }
    }
    first: $first
    after: $after
    orderBy: createdAt
  ) {
    nodes {
      id
      identifier
      title
      description
      priority
      state { name }
      branchName
      url
      labels { nodes { name } }
      relations { nodes { type relatedIssue { id identifier state { name } } } }
      inverseRelations { nodes { type issue { id identifier state { name } } } }
      createdAt
      updatedAt
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"#;

/// GraphQL query for fetching issues by states (S11.2).
const ISSUES_BY_STATES_QUERY: &str = r#"
query IssuesByStates($projectSlug: String!, $states: [String!]!, $first: Int!, $after: String) {
  issues(
    filter: {
      project: { slugId: { eq: $projectSlug } }
      state: { name: { in: $states } }
    }
    first: $first
    after: $after
  ) {
    nodes {
      id
      identifier
      title
      description
      priority
      state { name }
      branchName
      url
      labels { nodes { name } }
      relations { nodes { type relatedIssue { id identifier state { name } } } }
      inverseRelations { nodes { type issue { id identifier state { name } } } }
      createdAt
      updatedAt
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"#;

/// GraphQL query for fetching issue states by IDs (S11.2).
const ISSUE_STATES_BY_IDS_QUERY: &str = r#"
query IssueStatesByIds($ids: [ID!], $first: Int!) {
  issues(
    filter: { id: { in: $ids } }
    first: $first
  ) {
    nodes {
      id
      identifier
      title
      state { name }
      priority
      createdAt
      updatedAt
    }
  }
}
"#;

#[async_trait]
impl TrackerClient for LinearClient {
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>, TrackerError> {
        self.fetch_paginated_issues(
            CANDIDATE_ISSUES_QUERY,
            |cursor| {
                let mut vars = serde_json::json!({
                    "projectSlug": self.project_slug,
                    "first": PAGE_SIZE,
                });
                if let Some(c) = cursor {
                    vars.as_object_mut()
                        .unwrap()
                        .insert("after".into(), Value::String(c.into()));
                }
                vars
            },
            &["issues"],
        )
        .await
    }

    async fn fetch_issues_by_states(&self, states: &[String]) -> Result<Vec<Issue>, TrackerError> {
        if states.is_empty() {
            return Ok(vec![]);
        }

        self.fetch_paginated_issues(
            ISSUES_BY_STATES_QUERY,
            |cursor| {
                let mut vars = serde_json::json!({
                    "projectSlug": self.project_slug,
                    "states": states,
                    "first": PAGE_SIZE,
                });
                if let Some(c) = cursor {
                    vars.as_object_mut()
                        .unwrap()
                        .insert("after".into(), Value::String(c.into()));
                }
                vars
            },
            &["issues"],
        )
        .await
    }

    async fn fetch_issue_states_by_ids(
        &self,
        issue_ids: &[String],
    ) -> Result<Vec<Issue>, TrackerError> {
        if issue_ids.is_empty() {
            return Ok(vec![]);
        }

        let variables = serde_json::json!({
            "ids": issue_ids,
            "first": issue_ids.len(),
        });
        let data = self.graphql_query(ISSUE_STATES_BY_IDS_QUERY, variables).await?;

        let nodes = data
            .get("issues")
            .and_then(|i| i.get("nodes"))
            .and_then(|n| n.as_array())
            .ok_or_else(|| {
                TrackerError::UnknownPayload("missing 'issues.nodes' in response".into())
            })?;

        let mut issues = Vec::new();
        for node_val in nodes {
            if node_val.is_null() {
                continue;
            }
            if let Some(issue) = normalize_issue_minimal(node_val) {
                issues.push(issue);
            }
        }
        Ok(issues)
    }
}

/// Normalize a full Linear issue JSON node to domain Issue (Spec Section 11.3).
fn normalize_issue(v: &Value) -> Option<Issue> {
    let id = v.get("id")?.as_str()?.to_string();
    let identifier = v.get("identifier")?.as_str()?.to_string();
    let title = v.get("title")?.as_str()?.to_string();
    let description = v.get("description").and_then(|d| d.as_str()).map(String::from);

    // Priority: integer only; non-integer becomes None (S11.3)
    let priority = v
        .get("priority")
        .and_then(|p| p.as_i64())
        .map(|p| p as i32);

    let state = v
        .get("state")
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();

    let branch_name = v
        .get("branchName")
        .and_then(|b| b.as_str())
        .map(String::from);
    let url = v.get("url").and_then(|u| u.as_str()).map(String::from);

    // Labels: normalize to lowercase (S11.3)
    let labels = v
        .get("labels")
        .and_then(|l| l.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_default();

    // Blocked_by: derive from inverse relations where type is "blocks" (S11.3)
    let blocked_by = extract_blockers(v);

    // Timestamps: ISO-8601 parsing (S11.3)
    let created_at = v
        .get("createdAt")
        .and_then(|t| t.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());
    let updated_at = v
        .get("updatedAt")
        .and_then(|t| t.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    Some(Issue {
        id,
        identifier,
        title,
        description,
        priority,
        state,
        branch_name,
        url,
        labels,
        blocked_by,
        created_at,
        updated_at,
    })
}

/// Normalize a minimal issue from the nodes-by-ID query.
fn normalize_issue_minimal(v: &Value) -> Option<Issue> {
    let id = v.get("id")?.as_str()?.to_string();
    let identifier = v.get("identifier")?.as_str()?.to_string();
    let title = v
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    let state = v
        .get("state")
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();

    let priority = v
        .get("priority")
        .and_then(|p| p.as_i64())
        .map(|p| p as i32);

    let created_at = v
        .get("createdAt")
        .and_then(|t| t.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());
    let updated_at = v
        .get("updatedAt")
        .and_then(|t| t.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    Some(Issue {
        id,
        identifier,
        title,
        description: None,
        priority,
        state,
        branch_name: None,
        url: None,
        labels: vec![],
        blocked_by: vec![],
        created_at,
        updated_at,
    })
}

/// Extract blockers from inverse relations where type is "blocks" (S11.3).
fn extract_blockers(v: &Value) -> Vec<BlockerRef> {
    let mut blockers = Vec::new();

    // From inverseRelations: issue X blocks this issue
    if let Some(inv_nodes) = v
        .get("inverseRelations")
        .and_then(|r| r.get("nodes"))
        .and_then(|n| n.as_array())
    {
        for rel in inv_nodes {
            let rel_type = rel
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if rel_type == "blocks"
                && let Some(issue) = rel.get("issue")
            {
                blockers.push(BlockerRef {
                    id: issue.get("id").and_then(|i| i.as_str()).map(String::from),
                    identifier: issue
                        .get("identifier")
                        .and_then(|i| i.as_str())
                        .map(String::from),
                    state: issue
                        .get("state")
                        .and_then(|s| s.get("name"))
                        .and_then(|n| n.as_str())
                        .map(String::from),
                });
            }
        }
    }

    blockers
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

    #[test]
    fn normalize_full_issue() {
        let json = serde_json::json!({
            "id": "issue-1",
            "identifier": "PROJ-42",
            "title": "Fix the bug",
            "description": "Detailed description",
            "priority": 2,
            "state": { "name": "In Progress" },
            "branchName": "fix/proj-42",
            "url": "https://linear.app/team/PROJ-42",
            "labels": { "nodes": [{ "name": "BUG" }, { "name": "Urgent" }] },
            "relations": { "nodes": [] },
            "inverseRelations": { "nodes": [
                {
                    "type": "blocks",
                    "issue": { "id": "blocker-1", "identifier": "PROJ-10", "state": { "name": "Done" } }
                }
            ] },
            "createdAt": "2025-01-15T10:00:00.000Z",
            "updatedAt": "2025-01-16T10:00:00.000Z"
        });

        let issue = normalize_issue(&json).unwrap();
        assert_eq!(issue.id, "issue-1");
        assert_eq!(issue.identifier, "PROJ-42");
        assert_eq!(issue.title, "Fix the bug");
        assert_eq!(issue.description, Some("Detailed description".into()));
        assert_eq!(issue.priority, Some(2));
        assert_eq!(issue.state, "In Progress");
        assert_eq!(issue.branch_name, Some("fix/proj-42".into()));
        // Labels normalized to lowercase (S11.3)
        assert_eq!(issue.labels, vec!["bug", "urgent"]);
        // Blocker derived from inverse "blocks" relation (S11.3)
        assert_eq!(issue.blocked_by.len(), 1);
        assert_eq!(issue.blocked_by[0].identifier, Some("PROJ-10".into()));
        assert_eq!(issue.blocked_by[0].state, Some("Done".into()));
        assert!(issue.created_at.is_some());
        assert!(issue.updated_at.is_some());
    }

    #[test]
    fn normalize_non_integer_priority_becomes_none() {
        let json = serde_json::json!({
            "id": "issue-1",
            "identifier": "PROJ-42",
            "title": "Test",
            "priority": "high",
            "state": { "name": "Todo" }
        });
        let issue = normalize_issue(&json).unwrap();
        assert_eq!(issue.priority, None);
    }

    #[test]
    fn normalize_null_priority_becomes_none() {
        let json = serde_json::json!({
            "id": "issue-1",
            "identifier": "PROJ-42",
            "title": "Test",
            "priority": null,
            "state": { "name": "Todo" }
        });
        let issue = normalize_issue(&json).unwrap();
        assert_eq!(issue.priority, None);
    }

    #[test]
    fn normalize_labels_lowercase() {
        let json = serde_json::json!({
            "id": "issue-1",
            "identifier": "PROJ-42",
            "title": "Test",
            "state": { "name": "Todo" },
            "labels": { "nodes": [{ "name": "BUG" }, { "name": "FEATURE" }] }
        });
        let issue = normalize_issue(&json).unwrap();
        assert_eq!(issue.labels, vec!["bug", "feature"]);
    }

    #[test]
    fn normalize_blocker_from_inverse_blocks() {
        let json = serde_json::json!({
            "id": "issue-1",
            "identifier": "PROJ-42",
            "title": "Test",
            "state": { "name": "Todo" },
            "inverseRelations": { "nodes": [
                { "type": "blocks", "issue": { "id": "b1", "identifier": "PROJ-10", "state": { "name": "In Progress" } } },
                { "type": "related", "issue": { "id": "r1", "identifier": "PROJ-20", "state": { "name": "Todo" } } }
            ] }
        });
        let issue = normalize_issue(&json).unwrap();
        // Only "blocks" type should appear
        assert_eq!(issue.blocked_by.len(), 1);
        assert_eq!(issue.blocked_by[0].identifier, Some("PROJ-10".into()));
    }

    #[test]
    fn normalize_minimal_issue() {
        let json = serde_json::json!({
            "id": "issue-1",
            "identifier": "PROJ-42",
            "title": "Test",
            "state": { "name": "Todo" },
            "priority": 1,
            "createdAt": "2025-01-15T10:00:00.000Z"
        });
        let issue = normalize_issue_minimal(&json).unwrap();
        assert_eq!(issue.id, "issue-1");
        assert_eq!(issue.identifier, "PROJ-42");
        assert_eq!(issue.state, "Todo");
        assert_eq!(issue.priority, Some(1));
        assert!(issue.created_at.is_some());
    }

    #[test]
    fn normalize_missing_required_fields_returns_none() {
        // Missing identifier
        let json = serde_json::json!({ "id": "issue-1", "title": "Test" });
        assert!(normalize_issue(&json).is_none());
    }

    #[test]
    fn error_variants_are_distinct() {
        let errors: Vec<TrackerError> = vec![
            TrackerError::UnsupportedKind("x".into()),
            TrackerError::MissingApiKey,
            TrackerError::MissingProjectSlug,
            TrackerError::ApiRequest("x".into()),
            TrackerError::ApiStatus { status: 401, body: "x".into() },
            TrackerError::GraphqlErrors("x".into()),
            TrackerError::UnknownPayload("x".into()),
            TrackerError::MissingEndCursor,
        ];
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        assert!(msgs[0].starts_with("unsupported_tracker_kind"));
        assert!(msgs[1].starts_with("missing_tracker_api_key"));
        assert!(msgs[2].starts_with("missing_tracker_project_slug"));
        assert!(msgs[3].starts_with("linear_api_request"));
        assert!(msgs[4].starts_with("linear_api_status"));
        assert!(msgs[5].starts_with("linear_graphql_errors"));
        assert!(msgs[6].starts_with("linear_unknown_payload"));
        assert!(msgs[7].starts_with("linear_missing_end_cursor"));
    }

    #[test]
    fn iso8601_timestamp_parsing() {
        let json = serde_json::json!({
            "id": "issue-1",
            "identifier": "PROJ-42",
            "title": "Test",
            "state": { "name": "Todo" },
            "createdAt": "2025-01-15T10:30:00.000Z",
            "updatedAt": "invalid-date"
        });
        let issue = normalize_issue(&json).unwrap();
        assert!(issue.created_at.is_some());
        assert!(issue.updated_at.is_none()); // invalid date → None
    }

    // ─── Real Linear Integration Tests (S17.8) ───
    // These tests require LINEAR_API_KEY env var and a valid project.
    // Run with: cargo test -- --ignored
    // They are reported as "ignored" (skipped) when credentials are absent.

    /// Helper to get Linear credentials from env, or skip.
    fn get_real_linear_config() -> Option<(String, String)> {
        let api_key = std::env::var("LINEAR_API_KEY").ok()?;
        let project_slug = std::env::var("LINEAR_PROJECT_SLUG")
            .unwrap_or_else(|_| "symphony-test".into());
        if api_key.is_empty() {
            return None;
        }
        Some((api_key, project_slug))
    }

    #[tokio::test]
    #[ignore] // S17.8: skipped when credentials absent, reported as skipped
    async fn real_linear_graphql_query() {
        let (api_key, _) = get_real_linear_config()
            .expect("LINEAR_API_KEY must be set for real integration tests");

        let client = LinearClient::new(
            "https://api.linear.app/graphql".into(),
            api_key,
            "unused".into(),
            vec![],
        );

        // Simple viewer query to validate auth works
        let data = client
            .graphql_query("query { viewer { id name } }", serde_json::json!({}))
            .await
            .expect("real Linear API call should succeed");

        assert!(data.get("viewer").is_some(), "viewer field should be present");
        assert!(
            data["viewer"].get("id").is_some(),
            "viewer.id should be present"
        );
    }

    #[tokio::test]
    #[ignore] // S17.8: skipped when credentials absent
    async fn real_linear_fetch_issues() {
        let (api_key, project_slug) = get_real_linear_config()
            .expect("LINEAR_API_KEY must be set for real integration tests");

        let client = LinearClient::new(
            "https://api.linear.app/graphql".into(),
            api_key,
            project_slug,
            vec!["Todo".into(), "In Progress".into()],
        );

        // Fetch candidate issues — may return empty if project has no active issues
        let issues = client
            .fetch_candidate_issues()
            .await
            .expect("fetch_candidate_issues should succeed with valid credentials");

        // Validate each issue has required fields
        for issue in &issues {
            assert!(!issue.id.is_empty(), "issue.id should not be empty");
            assert!(!issue.identifier.is_empty(), "issue.identifier should not be empty");
            assert!(!issue.title.is_empty(), "issue.title should not be empty");
            assert!(!issue.state.is_empty(), "issue.state should not be empty");
        }
    }

    #[tokio::test]
    #[ignore] // S17.8: skipped when credentials absent
    async fn real_linear_invalid_key_returns_error() {
        let client = LinearClient::new(
            "https://api.linear.app/graphql".into(),
            "lin_api_invalid_key_12345".into(),
            "test-proj".into(),
            vec!["Todo".into()],
        );

        let result = client
            .graphql_query("query { viewer { id } }", serde_json::json!({}))
            .await;

        assert!(result.is_err(), "invalid API key should produce an error");
    }
}
