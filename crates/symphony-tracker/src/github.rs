// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! GitHub Issues tracker adapter.
//!
//! Uses the GitHub REST API to fetch issues and normalize them into
//! the Symphony domain model.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use symphony_core::Issue;

use crate::{TrackerClient, TrackerError};

const PER_PAGE: u32 = 100;
const MAX_DESCRIPTION_LEN: usize = 4000;

/// GitHub REST API client for issue tracking.
pub struct GithubClient {
    api_base: String,
    api_token: String,
    repo_owner: String,
    repo_name: String,
    active_states: Vec<String>,
    http: reqwest::Client,
}

/// A single GitHub issue from the REST API.
#[derive(Debug, Deserialize)]
struct GithubIssue {
    node_id: String,
    number: u64,
    title: String,
    body: Option<String>,
    state: String,
    html_url: String,
    labels: Vec<GithubLabel>,
    pull_request: Option<serde_json::Value>,
    created_at: String,
    updated_at: String,
}

/// A GitHub label.
#[derive(Debug, Deserialize)]
struct GithubLabel {
    name: String,
}

impl GithubClient {
    /// Create a new GitHub tracker client.
    ///
    /// `project_slug` should be in `owner/repo` format.
    pub fn new(
        api_token: String,
        repo_owner: String,
        repo_name: String,
        active_states: Vec<String>,
    ) -> Self {
        Self {
            api_base: "https://api.github.com".into(),
            api_token,
            repo_owner,
            repo_name,
            active_states,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(30_000))
                .user_agent("symphony-tracker/0.1")
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    /// Build from a `project_slug` in `owner/repo` format.
    pub fn from_slug(
        api_token: String,
        project_slug: &str,
        active_states: Vec<String>,
    ) -> Result<Self, TrackerError> {
        let (owner, repo) = parse_owner_repo(project_slug)?;
        Ok(Self::new(api_token, owner, repo, active_states))
    }

    /// Full identifier like `owner/repo#42`.
    fn identifier(&self, number: u64) -> String {
        format!("{}/{}#{}", self.repo_owner, self.repo_name, number)
    }

    /// Execute a GET request against the GitHub API.
    async fn get(&self, url: &str) -> Result<reqwest::Response, TrackerError> {
        let response = self
            .http
            .get(url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| TrackerError::GithubApiRequest(e.to_string()))?;

        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".into());
            return Err(TrackerError::GithubApiStatus { status, body });
        }

        Ok(response)
    }

    /// Fetch issues with pagination. Returns all pages.
    async fn fetch_issues_paged(
        &self,
        github_state: &str,
    ) -> Result<Vec<GithubIssue>, TrackerError> {
        let mut all = Vec::new();
        let mut page = 1u32;

        loop {
            let url = format!(
                "{}/repos/{}/{}/issues?state={}&per_page={}&page={}&sort=created&direction=asc",
                self.api_base, self.repo_owner, self.repo_name, github_state, PER_PAGE, page
            );

            tracing::debug!(url = %url, "fetching GitHub issues page");
            let response = self.get(&url).await?;

            // Check Link header for next page before consuming body
            let has_next = response
                .headers()
                .get("link")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|link| link.contains("rel=\"next\""));

            let issues: Vec<GithubIssue> = response
                .json()
                .await
                .map_err(|e| TrackerError::GithubApiRequest(format!("json parse: {e}")))?;

            if issues.is_empty() {
                break;
            }

            all.extend(issues);

            if !has_next {
                break;
            }

            page += 1;
        }

        Ok(all)
    }

    /// Fetch a single issue by number.
    async fn fetch_issue_by_number(&self, number: u64) -> Result<GithubIssue, TrackerError> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.api_base, self.repo_owner, self.repo_name, number
        );

        tracing::debug!(url = %url, "fetching single GitHub issue");
        let response = self.get(&url).await?;

        response
            .json()
            .await
            .map_err(|e| TrackerError::GithubApiRequest(format!("json parse: {e}")))
    }

    /// Normalize a `GithubIssue` into the Symphony `Issue` type.
    fn normalize(&self, gh: &GithubIssue) -> Issue {
        let labels: Vec<String> = gh.labels.iter().map(|l| l.name.to_lowercase()).collect();
        let state = derive_state(&gh.state, &labels, &self.active_states);

        let description = gh.body.as_deref().map(|b| {
            if b.len() > MAX_DESCRIPTION_LEN {
                b[..MAX_DESCRIPTION_LEN].to_string()
            } else {
                b.to_string()
            }
        });

        let created_at = gh.created_at.parse::<DateTime<Utc>>().ok();
        let updated_at = gh.updated_at.parse::<DateTime<Utc>>().ok();

        Issue {
            id: gh.node_id.clone(),
            identifier: self.identifier(gh.number),
            title: gh.title.clone(),
            description,
            priority: None,
            state,
            branch_name: None,
            url: Some(gh.html_url.clone()),
            labels,
            blocked_by: vec![],
            created_at,
            updated_at,
        }
    }
}

/// Parse `owner/repo` from a project slug string.
pub fn parse_owner_repo(slug: &str) -> Result<(String, String), TrackerError> {
    let parts: Vec<&str> = slug.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(TrackerError::GithubApiRequest(format!(
            "invalid project_slug format '{slug}': expected 'owner/repo'"
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Derive the Symphony state from GitHub's open/closed state and labels.
///
/// If the issue is closed, returns "closed".
/// If the issue is open and has a label matching one of the active states
/// (case-insensitive), returns that label.
/// Otherwise returns "open".
pub fn derive_state(github_state: &str, labels: &[String], active_states: &[String]) -> String {
    if github_state == "closed" {
        return "closed".to_string();
    }

    // Check if any label matches an active state (both already lowercase)
    let active_lower: Vec<String> = active_states
        .iter()
        .map(|s| s.trim().to_lowercase())
        .collect();

    for label in labels {
        if active_lower.contains(label) {
            return label.clone();
        }
    }

    "open".to_string()
}

/// Return true if the issue JSON represents a pull request (has `pull_request` field).
fn is_pull_request(gh: &GithubIssue) -> bool {
    gh.pull_request.is_some()
}

/// Terminal state keywords that map to GitHub `state=closed`.
const TERMINAL_KEYWORDS: &[&str] = &["closed", "done", "canceled", "cancelled"];

/// Check whether any of the given states are terminal (closed-like).
fn has_terminal_states(states: &[String]) -> bool {
    states.iter().any(|s| {
        let lower = s.trim().to_lowercase();
        TERMINAL_KEYWORDS.contains(&lower.as_str())
    })
}

/// Check whether any of the given states are open-like.
fn has_open_states(states: &[String]) -> bool {
    states
        .iter()
        .any(|s| !TERMINAL_KEYWORDS.contains(&s.trim().to_lowercase().as_str()))
}

#[async_trait]
impl TrackerClient for GithubClient {
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>, TrackerError> {
        let gh_issues = self.fetch_issues_paged("open").await?;

        let issues: Vec<Issue> = gh_issues
            .iter()
            .filter(|gh| !is_pull_request(gh))
            .map(|gh| self.normalize(gh))
            .collect();

        tracing::info!(count = issues.len(), "fetched GitHub candidate issues");
        Ok(issues)
    }

    async fn fetch_issues_by_states(&self, states: &[String]) -> Result<Vec<Issue>, TrackerError> {
        if states.is_empty() {
            return Ok(vec![]);
        }

        let mut all_issues = Vec::new();

        // Fetch open issues if any state is non-terminal
        if has_open_states(states) {
            let open = self.fetch_issues_paged("open").await?;
            all_issues.extend(open);
        }

        // Fetch closed issues if any state is terminal
        if has_terminal_states(states) {
            let closed = self.fetch_issues_paged("closed").await?;
            all_issues.extend(closed);
        }

        let issues: Vec<Issue> = all_issues
            .iter()
            .filter(|gh| !is_pull_request(gh))
            .map(|gh| self.normalize(gh))
            .collect();

        // Filter to only issues whose derived state matches one of the requested states
        let states_lower: Vec<String> = states.iter().map(|s| s.trim().to_lowercase()).collect();

        let filtered: Vec<Issue> = issues
            .into_iter()
            .filter(|i| states_lower.contains(&i.state.trim().to_lowercase()))
            .collect();

        tracing::info!(count = filtered.len(), "fetched GitHub issues by states");
        Ok(filtered)
    }

    async fn fetch_issue_states_by_ids(
        &self,
        issue_ids: &[String],
    ) -> Result<Vec<Issue>, TrackerError> {
        if issue_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut issues = Vec::new();

        for id in issue_ids {
            // Extract issue number from identifier like "owner/repo#42"
            let number = extract_issue_number(id);
            match number {
                Some(num) => match self.fetch_issue_by_number(num).await {
                    Ok(gh) => {
                        if !is_pull_request(&gh) {
                            issues.push(self.normalize(&gh));
                        }
                    }
                    Err(e) => {
                        tracing::warn!(issue_id = %id, error = %e, "failed to fetch GitHub issue");
                    }
                },
                None => {
                    tracing::warn!(issue_id = %id, "cannot extract issue number from identifier");
                }
            }
        }

        Ok(issues)
    }
}

/// Extract the issue number from an identifier.
///
/// Accepts formats like:
/// - `owner/repo#42` → 42
/// - `42` → 42
/// - `#42` → 42
fn extract_issue_number(id: &str) -> Option<u64> {
    // Try owner/repo#N format
    if let Some(hash_pos) = id.rfind('#') {
        return id[hash_pos + 1..].parse().ok();
    }

    // Try plain number
    id.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client() -> GithubClient {
        GithubClient::new(
            "test-token".into(),
            "broomva".into(),
            "symphony".into(),
            vec!["open".into(), "in progress".into()],
        )
    }

    #[test]
    fn normalize_github_issue() {
        let client = make_client();

        let gh = GithubIssue {
            node_id: "MDU6SXNzdWUx".into(),
            number: 42,
            title: "Fix the bug".into(),
            body: Some("Detailed description of the bug".into()),
            state: "open".into(),
            html_url: "https://github.com/broomva/symphony/issues/42".into(),
            labels: vec![
                GithubLabel { name: "Bug".into() },
                GithubLabel {
                    name: "In Progress".into(),
                },
            ],
            pull_request: None,
            created_at: "2025-06-15T10:00:00Z".into(),
            updated_at: "2025-06-16T12:00:00Z".into(),
        };

        let issue = client.normalize(&gh);
        assert_eq!(issue.id, "MDU6SXNzdWUx");
        assert_eq!(issue.identifier, "broomva/symphony#42");
        assert_eq!(issue.title, "Fix the bug");
        assert_eq!(
            issue.description,
            Some("Detailed description of the bug".into())
        );
        assert_eq!(issue.priority, None);
        // "in progress" label matches active state
        assert_eq!(issue.state, "in progress");
        assert_eq!(issue.branch_name, None);
        assert_eq!(
            issue.url,
            Some("https://github.com/broomva/symphony/issues/42".into())
        );
        assert_eq!(issue.labels, vec!["bug", "in progress"]);
        assert!(issue.blocked_by.is_empty());
        assert!(issue.created_at.is_some());
        assert!(issue.updated_at.is_some());
    }

    #[test]
    fn normalize_filters_pull_requests() {
        let prs: Vec<GithubIssue> = vec![
            GithubIssue {
                node_id: "issue-1".into(),
                number: 1,
                title: "Real issue".into(),
                body: None,
                state: "open".into(),
                html_url: "https://github.com/o/r/issues/1".into(),
                labels: vec![],
                pull_request: None,
                created_at: "2025-01-01T00:00:00Z".into(),
                updated_at: "2025-01-01T00:00:00Z".into(),
            },
            GithubIssue {
                node_id: "pr-2".into(),
                number: 2,
                title: "A pull request".into(),
                body: None,
                state: "open".into(),
                html_url: "https://github.com/o/r/pull/2".into(),
                labels: vec![],
                pull_request: Some(serde_json::json!({})),
                created_at: "2025-01-01T00:00:00Z".into(),
                updated_at: "2025-01-01T00:00:00Z".into(),
            },
        ];

        let filtered: Vec<&GithubIssue> = prs.iter().filter(|gh| !is_pull_request(gh)).collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].node_id, "issue-1");
    }

    #[test]
    fn parse_owner_repo_valid() {
        let (owner, repo) = parse_owner_repo("broomva/symphony").unwrap();
        assert_eq!(owner, "broomva");
        assert_eq!(repo, "symphony");
    }

    #[test]
    fn parse_owner_repo_with_nested_slash() {
        // Only first slash is the delimiter
        let (owner, repo) = parse_owner_repo("org/repo-with-dashes").unwrap();
        assert_eq!(owner, "org");
        assert_eq!(repo, "repo-with-dashes");
    }

    #[test]
    fn parse_owner_repo_invalid() {
        assert!(parse_owner_repo("noslash").is_err());
        assert!(parse_owner_repo("/repo").is_err());
        assert!(parse_owner_repo("owner/").is_err());
        assert!(parse_owner_repo("").is_err());
    }

    #[test]
    fn state_from_labels_matches_active() {
        let labels = vec!["bug".into(), "in progress".into()];
        let active = vec!["open".into(), "in progress".into()];
        let state = derive_state("open", &labels, &active);
        assert_eq!(state, "in progress");
    }

    #[test]
    fn state_from_labels_no_match_returns_open() {
        let labels = vec!["bug".into(), "enhancement".into()];
        let active = vec!["open".into(), "in progress".into()];
        let state = derive_state("open", &labels, &active);
        assert_eq!(state, "open");
    }

    #[test]
    fn state_from_labels_closed_always_closed() {
        let labels = vec!["in progress".into()];
        let active = vec!["open".into(), "in progress".into()];
        let state = derive_state("closed", &labels, &active);
        assert_eq!(state, "closed");
    }

    #[test]
    fn empty_issues_returns_empty() {
        let client = make_client();
        let empty: Vec<GithubIssue> = vec![];
        let result: Vec<Issue> = empty
            .iter()
            .filter(|gh| !is_pull_request(gh))
            .map(|gh| client.normalize(gh))
            .collect();
        assert!(result.is_empty());
    }

    #[test]
    fn extract_issue_number_from_identifier() {
        assert_eq!(extract_issue_number("broomva/symphony#42"), Some(42));
        assert_eq!(extract_issue_number("org/repo#1"), Some(1));
        assert_eq!(extract_issue_number("#7"), Some(7));
        assert_eq!(extract_issue_number("123"), Some(123));
        assert_eq!(extract_issue_number("invalid"), None);
    }

    #[test]
    fn description_truncation() {
        let client = make_client();
        let long_body = "x".repeat(5000);
        let gh = GithubIssue {
            node_id: "id".into(),
            number: 1,
            title: "T".into(),
            body: Some(long_body),
            state: "open".into(),
            html_url: "https://github.com/o/r/issues/1".into(),
            labels: vec![],
            pull_request: None,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        };
        let issue = client.normalize(&gh);
        assert_eq!(
            issue.description.as_ref().map(|d| d.len()),
            Some(MAX_DESCRIPTION_LEN)
        );
    }

    #[test]
    fn terminal_state_detection() {
        assert!(has_terminal_states(&["closed".into()]));
        assert!(has_terminal_states(&["Done".into()]));
        assert!(has_terminal_states(&["open".into(), "Canceled".into()]));
        assert!(!has_terminal_states(&["open".into(), "in progress".into()]));
    }

    #[tokio::test]
    async fn empty_ids_returns_empty() {
        let client = make_client();
        let result = client.fetch_issue_states_by_ids(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn empty_states_returns_empty() {
        let client = make_client();
        let result = client.fetch_issues_by_states(&[]).await.unwrap();
        assert!(result.is_empty());
    }
}
