// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Markdown file-based issue tracker adapter.
//!
//! Reads `.md` files from a local directory, parsing YAML front matter
//! as issue metadata. State transitions are written back to the front matter.
//! No external API or credentials required.
//!
//! ## Lago Integration
//!
//! When `endpoint` is configured, every state transition is journaled as a
//! Lago-compatible JSONL entry in `{issues_dir}/.journal.jsonl`. If the
//! endpoint points to a running Lago daemon (`http://host:port`), a session
//! is created on first use and the journal is also forwarded via HTTP.
//! The journal uses `EventPayload::Custom` schema for forward-compatibility.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use symphony_core::{BlockerRef, Issue};

use crate::{TrackerClient, TrackerError};

const MAX_DESCRIPTION_LEN: usize = 4000;

/// Markdown file-based tracker client with optional Lago journaling.
///
/// Issues are `.md` files in a directory. Each file has YAML front matter
/// with issue metadata (id, title, state, priority, labels, blocked_by)
/// and a markdown body used as the issue description.
///
/// State transitions are optionally journaled to a local JSONL file using
/// Lago's event schema, enabling audit trails and future Lago import.
pub struct MarkdownClient {
    /// Directory containing issue `.md` files.
    issues_dir: std::path::PathBuf,
    /// Active states from WORKFLOW.md config.
    active_states: Vec<String>,
    /// Optional Lago journal for audit trail.
    journal: Option<Journal>,
}

/// YAML front matter schema for a markdown issue file.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct IssueFrontMatter {
    id: String,
    title: String,
    state: String,
    #[serde(default)]
    priority: Option<i32>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    blocked_by: Vec<BlockerFrontMatter>,
    #[serde(default)]
    branch_name: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct BlockerFrontMatter {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

/// JSONL audit journal using Lago's EventPayload::Custom schema.
///
/// Each entry is a single JSON line with the structure:
/// ```json
/// {
///   "event_id": "ulid",
///   "session_id": "symphony",
///   "timestamp": "2026-03-19T10:00:00Z",
///   "payload": {
///     "type": "Custom",
///     "event_type": "symphony.tracker.state_transition",
///     "data": { "issue_id": "...", "from_state": "...", "to_state": "..." }
///   }
/// }
/// ```
pub struct Journal {
    /// Path to the `.journal.jsonl` file.
    journal_path: std::path::PathBuf,
    /// Optional Lago HTTP endpoint for session creation and forwarding.
    lago_endpoint: Option<String>,
}

/// A single journal entry, compatible with Lago's EventEnvelope schema.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct JournalEntry {
    event_id: String,
    session_id: String,
    branch_id: String,
    timestamp: String,
    payload: JournalPayload,
    #[serde(default)]
    metadata: std::collections::HashMap<String, String>,
}

/// Lago-compatible EventPayload::Custom structure.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct JournalPayload {
    #[serde(rename = "type")]
    payload_type: String,
    event_type: String,
    data: serde_json::Value,
}

impl Journal {
    /// Create a new journal. The JSONL file is created at `{issues_dir}/.journal.jsonl`.
    pub fn new(issues_dir: &std::path::Path, lago_endpoint: Option<String>) -> Self {
        Self {
            journal_path: issues_dir.join(".journal.jsonl"),
            lago_endpoint,
        }
    }

    /// Append a state transition event to the journal.
    pub fn log_state_transition(
        &self,
        issue_id: &str,
        from_state: &str,
        to_state: &str,
        issue_title: &str,
    ) -> Result<(), TrackerError> {
        let entry = JournalEntry {
            event_id: generate_ulid_like(),
            session_id: "symphony".into(),
            branch_id: "main".into(),
            timestamp: Utc::now().to_rfc3339(),
            payload: JournalPayload {
                payload_type: "Custom".into(),
                event_type: "symphony.tracker.state_transition".into(),
                data: serde_json::json!({
                    "issue_id": issue_id,
                    "issue_title": issue_title,
                    "from_state": from_state,
                    "to_state": to_state,
                }),
            },
            metadata: std::collections::HashMap::new(),
        };

        let line = serde_json::to_string(&entry)
            .map_err(|e| TrackerError::MarkdownIoError(format!("journal serialize error: {e}")))?;

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.journal_path)
            .map_err(|e| {
                TrackerError::MarkdownIoError(format!(
                    "cannot open journal {}: {e}",
                    self.journal_path.display()
                ))
            })?;

        writeln!(file, "{line}").map_err(|e| {
            TrackerError::MarkdownIoError(format!(
                "cannot write to journal {}: {e}",
                self.journal_path.display()
            ))
        })?;

        tracing::debug!(
            issue_id = %issue_id,
            from_state = %from_state,
            to_state = %to_state,
            journal = %self.journal_path.display(),
            "journaled state transition"
        );

        Ok(())
    }

    /// Log an issue scan event (records what the tracker saw at poll time).
    pub fn log_scan(&self, issues: &[Issue]) -> Result<(), TrackerError> {
        let entry = JournalEntry {
            event_id: generate_ulid_like(),
            session_id: "symphony".into(),
            branch_id: "main".into(),
            timestamp: Utc::now().to_rfc3339(),
            payload: JournalPayload {
                payload_type: "Custom".into(),
                event_type: "symphony.tracker.scan".into(),
                data: serde_json::json!({
                    "issue_count": issues.len(),
                    "issues": issues.iter().map(|i| serde_json::json!({
                        "id": i.id,
                        "title": i.title,
                        "state": i.state,
                    })).collect::<Vec<_>>(),
                }),
            },
            metadata: std::collections::HashMap::new(),
        };

        let line = serde_json::to_string(&entry)
            .map_err(|e| TrackerError::MarkdownIoError(format!("journal serialize error: {e}")))?;

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.journal_path)
            .map_err(|e| {
                TrackerError::MarkdownIoError(format!(
                    "cannot open journal {}: {e}",
                    self.journal_path.display()
                ))
            })?;

        writeln!(file, "{line}").map_err(|e| {
            TrackerError::MarkdownIoError(format!(
                "cannot write to journal {}: {e}",
                self.journal_path.display()
            ))
        })?;

        Ok(())
    }

    /// Read all journal entries (for inspection/debugging).
    pub fn read_entries(&self) -> Result<Vec<JournalEntry>, TrackerError> {
        if !self.journal_path.exists() {
            return Ok(vec![]);
        }

        let content = std::fs::read_to_string(&self.journal_path).map_err(|e| {
            TrackerError::MarkdownIoError(format!(
                "cannot read journal {}: {e}",
                self.journal_path.display()
            ))
        })?;

        let mut entries = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<JournalEntry>(line) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    tracing::warn!(error = %e, "skipping malformed journal entry");
                }
            }
        }

        Ok(entries)
    }

    /// Check if Lago endpoint is configured and reachable.
    pub async fn check_lago(&self) -> Option<String> {
        let endpoint = self.lago_endpoint.as_ref()?;

        match reqwest::Client::new()
            .get(format!("{endpoint}/healthz"))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(endpoint = %endpoint, "lago daemon is reachable");
                Some(endpoint.clone())
            }
            Ok(resp) => {
                tracing::warn!(
                    endpoint = %endpoint,
                    status = %resp.status(),
                    "lago daemon returned non-success"
                );
                None
            }
            Err(e) => {
                tracing::debug!(
                    endpoint = %endpoint,
                    error = %e,
                    "lago daemon not reachable, journal-only mode"
                );
                None
            }
        }
    }

    /// Create a Lago session for this Symphony project (if endpoint is reachable).
    pub async fn ensure_lago_session(&self) -> Option<String> {
        let endpoint = self.check_lago().await?;

        let body = serde_json::json!({
            "name": "symphony-markdown-tracker",
            "model": "symphony",
            "params": {}
        });

        match reqwest::Client::new()
            .post(format!("{endpoint}/v1/sessions"))
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    let session_id = data
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    tracing::info!(
                        session_id = %session_id,
                        "created lago session for symphony tracker"
                    );
                    return Some(session_id.to_string());
                }
                None
            }
            Ok(resp) => {
                tracing::warn!(
                    status = %resp.status(),
                    "failed to create lago session"
                );
                None
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to create lago session");
                None
            }
        }
    }
}

/// Generate a ULID-like unique ID (timestamp + random suffix).
/// Uses epoch milliseconds + 6 random hex chars for uniqueness.
fn generate_ulid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let rand_suffix: u32 = (ts as u32).wrapping_mul(2654435761); // Knuth hash for fast entropy
    format!("{ts:013x}-{rand_suffix:08x}")
}

impl MarkdownClient {
    /// Create a markdown tracker without journaling.
    pub fn new(issues_dir: std::path::PathBuf, active_states: Vec<String>) -> Self {
        Self {
            issues_dir,
            active_states,
            journal: None,
        }
    }

    /// Create a markdown tracker with Lago-compatible JSONL journaling.
    ///
    /// The journal file is created at `{issues_dir}/.journal.jsonl`.
    /// If `lago_endpoint` is provided (e.g., `http://localhost:8080`),
    /// the tracker will also attempt to create a Lago session on startup.
    pub fn with_journal(
        issues_dir: std::path::PathBuf,
        active_states: Vec<String>,
        lago_endpoint: Option<String>,
    ) -> Self {
        let journal = Some(Journal::new(&issues_dir, lago_endpoint));
        Self {
            issues_dir,
            active_states,
            journal,
        }
    }

    /// Scan the issues directory and parse all `.md` files into Issues.
    fn read_all_issues(&self) -> Result<Vec<Issue>, TrackerError> {
        let dir = &self.issues_dir;
        if !dir.exists() {
            return Err(TrackerError::MarkdownIoError(format!(
                "issues directory does not exist: {}",
                dir.display()
            )));
        }

        let entries = std::fs::read_dir(dir).map_err(|e| {
            TrackerError::MarkdownIoError(format!("cannot read directory {}: {e}", dir.display()))
        })?;

        let mut issues = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| {
                TrackerError::MarkdownIoError(format!("directory entry error: {e}"))
            })?;

            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                match self.parse_issue_file(&path) {
                    Ok(issue) => issues.push(issue),
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "skipping malformed issue file"
                        );
                    }
                }
            }
        }

        Ok(issues)
    }

    /// Parse a single `.md` file into an Issue.
    fn parse_issue_file(&self, path: &std::path::Path) -> Result<Issue, TrackerError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            TrackerError::MarkdownIoError(format!("cannot read {}: {e}", path.display()))
        })?;

        let (front_matter, body) = parse_front_matter(&content).ok_or_else(|| {
            TrackerError::MarkdownParseError(format!(
                "missing or invalid YAML front matter in {}",
                path.display()
            ))
        })?;

        let fm: IssueFrontMatter = serde_yaml::from_str(front_matter).map_err(|e| {
            TrackerError::MarkdownParseError(format!(
                "invalid front matter in {}: {e}",
                path.display()
            ))
        })?;

        Ok(normalize_issue(&fm, body))
    }

    /// Write a state change back to the issue file's front matter.
    /// If a journal is configured, also logs the transition.
    fn write_state(&self, issue_id: &str, new_state: &str) -> Result<(), TrackerError> {
        let dir = &self.issues_dir;
        let entries = std::fs::read_dir(dir).map_err(|e| {
            TrackerError::MarkdownIoError(format!("cannot read directory {}: {e}", dir.display()))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                TrackerError::MarkdownIoError(format!("directory entry error: {e}"))
            })?;
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "md") {
                continue;
            }

            let content = std::fs::read_to_string(&path).map_err(|e| {
                TrackerError::MarkdownIoError(format!("cannot read {}: {e}", path.display()))
            })?;

            let Some((fm_str, _body)) = parse_front_matter(&content) else {
                continue;
            };

            // Check if this file matches the issue id
            let fm: IssueFrontMatter = match serde_yaml::from_str(fm_str) {
                Ok(fm) => fm,
                Err(_) => continue,
            };

            if fm.id != issue_id {
                continue;
            }

            let old_state = fm.state.clone();
            let title = fm.title.clone();

            // Rewrite the front matter with updated state and updated_at
            let updated_content =
                rewrite_state_in_front_matter(&content, new_state).ok_or_else(|| {
                    TrackerError::MarkdownIoError(format!(
                        "failed to rewrite state in {}",
                        path.display()
                    ))
                })?;

            std::fs::write(&path, updated_content).map_err(|e| {
                TrackerError::MarkdownIoError(format!("cannot write {}: {e}", path.display()))
            })?;

            // Journal the state transition (best-effort, don't fail the write)
            if let Some(journal) = &self.journal
                && let Err(e) =
                    journal.log_state_transition(issue_id, &old_state, new_state, &title)
            {
                tracing::warn!(error = %e, "failed to journal state transition");
            }

            tracing::info!(
                issue_id = %issue_id,
                from_state = %old_state,
                target_state = %new_state,
                path = %path.display(),
                "transitioned markdown issue state"
            );
            return Ok(());
        }

        tracing::warn!(
            issue_id = %issue_id,
            target_state = %new_state,
            "no markdown file found for issue, skipping state transition"
        );
        Ok(())
    }
}

#[async_trait]
impl TrackerClient for MarkdownClient {
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>, TrackerError> {
        let issues_dir = self.issues_dir.clone();
        let active_states = self.active_states.clone();
        let journal_path = self.journal.as_ref().map(|j| j.journal_path.clone());
        let lago_endpoint = self.journal.as_ref().and_then(|j| j.lago_endpoint.clone());

        let issues = tokio::task::spawn_blocking(move || {
            let mc = if journal_path.is_some() {
                MarkdownClient::with_journal(issues_dir, active_states, lago_endpoint)
            } else {
                MarkdownClient::new(issues_dir, active_states)
            };
            let issues = mc.read_all_issues()?;

            // Journal the scan (best-effort)
            if let Some(journal) = &mc.journal
                && let Err(e) = journal.log_scan(&issues)
            {
                tracing::warn!(error = %e, "failed to journal scan");
            }

            Ok::<Vec<Issue>, TrackerError>(issues)
        })
        .await
        .map_err(|e| TrackerError::MarkdownIoError(format!("spawn_blocking join: {e}")))?;

        let issues = issues?;
        tracing::info!(count = issues.len(), "fetched markdown candidate issues");
        Ok(issues)
    }

    async fn fetch_issues_by_states(&self, states: &[String]) -> Result<Vec<Issue>, TrackerError> {
        if states.is_empty() {
            return Ok(vec![]);
        }

        let all = self.fetch_candidate_issues().await?;
        let states_lower: Vec<String> = states.iter().map(|s| s.trim().to_lowercase()).collect();

        let filtered: Vec<Issue> = all
            .into_iter()
            .filter(|i| states_lower.contains(&i.state.trim().to_lowercase()))
            .collect();

        tracing::info!(count = filtered.len(), "fetched markdown issues by states");
        Ok(filtered)
    }

    async fn fetch_issue_states_by_ids(
        &self,
        issue_ids: &[String],
    ) -> Result<Vec<Issue>, TrackerError> {
        if issue_ids.is_empty() {
            return Ok(vec![]);
        }

        let all = self.fetch_candidate_issues().await?;
        let filtered: Vec<Issue> = all
            .into_iter()
            .filter(|i| issue_ids.contains(&i.id) || issue_ids.contains(&i.identifier))
            .collect();

        Ok(filtered)
    }

    async fn set_issue_state(&self, issue_id: &str, state: &str) -> Result<(), TrackerError> {
        let id = issue_id.to_string();
        let state = state.to_string();
        let dir = self.issues_dir.clone();
        let journal_path = self.journal.as_ref().map(|j| j.journal_path.clone());
        let lago_endpoint = self.journal.as_ref().and_then(|j| j.lago_endpoint.clone());

        tokio::task::spawn_blocking(move || {
            let mc = if journal_path.is_some() {
                MarkdownClient::with_journal(dir, vec![], lago_endpoint)
            } else {
                MarkdownClient::new(dir, vec![])
            };
            mc.write_state(&id, &state)
        })
        .await
        .map_err(|e| TrackerError::MarkdownIoError(format!("spawn_blocking join: {e}")))?
    }
}

/// Normalize front matter + body into a Symphony Issue.
fn normalize_issue(fm: &IssueFrontMatter, body: &str) -> Issue {
    let description = if body.is_empty() {
        None
    } else if body.len() > MAX_DESCRIPTION_LEN {
        Some(body[..MAX_DESCRIPTION_LEN].to_string())
    } else {
        Some(body.to_string())
    };

    let labels: Vec<String> = fm.labels.iter().map(|l| l.to_lowercase()).collect();

    let blocked_by: Vec<BlockerRef> = fm
        .blocked_by
        .iter()
        .map(|b| BlockerRef {
            id: b.id.clone(),
            identifier: b.identifier.clone(),
            state: b.state.clone(),
        })
        .collect();

    let created_at = fm
        .created_at
        .as_deref()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());
    let updated_at = fm
        .updated_at
        .as_deref()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    Issue {
        id: fm.id.clone(),
        identifier: fm.id.clone(), // markdown issues use id as identifier
        title: fm.title.clone(),
        description,
        priority: fm.priority,
        state: fm.state.clone(),
        branch_name: fm.branch_name.clone(),
        url: fm.url.clone(),
        labels,
        blocked_by,
        created_at,
        updated_at,
    }
}

/// Split content into (front_matter_str, body_str).
/// Returns None if no valid front matter delimiters found.
fn parse_front_matter(content: &str) -> Option<(&str, &str)> {
    let after_first = content.strip_prefix("---")?;
    let end_idx = after_first.find("\n---")?;
    let fm = &after_first[..end_idx];
    let rest = &after_first[end_idx + 4..]; // skip \n---
    Some((fm.trim(), rest.trim()))
}

/// Rewrite the `state:` line in YAML front matter and update `updated_at`.
fn rewrite_state_in_front_matter(content: &str, new_state: &str) -> Option<String> {
    let after_first = content.strip_prefix("---")?;
    let end_idx = after_first.find("\n---")?;
    let fm = &after_first[..end_idx];
    let rest = &after_first[end_idx..]; // includes \n---

    let now = Utc::now().to_rfc3339();

    let mut new_fm_lines = Vec::new();
    let mut state_found = false;
    let mut updated_at_found = false;

    for line in fm.lines() {
        if line.starts_with("state:") || line.starts_with("state :") {
            new_fm_lines.push(format!("state: {new_state}"));
            state_found = true;
        } else if line.starts_with("updated_at:") || line.starts_with("updated_at :") {
            new_fm_lines.push(format!("updated_at: \"{now}\""));
            updated_at_found = true;
        } else {
            new_fm_lines.push(line.to_string());
        }
    }

    if !state_found {
        return None;
    }

    if !updated_at_found {
        new_fm_lines.push(format!("updated_at: \"{now}\""));
    }

    Some(format!("---\n{}{}", new_fm_lines.join("\n"), rest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_issue(dir: &std::path::Path, filename: &str, content: &str) {
        let path = dir.join(filename);
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    const SAMPLE_ISSUE: &str = r#"---
id: TASK-001
title: Fix the auth bug
state: Todo
priority: 1
labels:
  - bug
  - auth
blocked_by: []
created_at: "2026-01-15T10:00:00Z"
updated_at: "2026-01-16T12:00:00Z"
---

The auth middleware stores session tokens in a non-compliant way.
Fix it to use encrypted cookies instead."#;

    const ISSUE_WITH_BLOCKERS: &str = r#"---
id: TASK-002
title: Deploy new auth
state: Todo
priority: 2
labels:
  - ops
blocked_by:
  - id: TASK-001
    identifier: TASK-001
    state: Todo
---

Deploy after TASK-001 is done."#;

    const DONE_ISSUE: &str = r#"---
id: TASK-003
title: Write README
state: Done
priority: 3
labels:
  - docs
---

Already completed."#;

    #[test]
    fn parse_front_matter_valid() {
        let (fm, body) = parse_front_matter(SAMPLE_ISSUE).unwrap();
        assert!(fm.contains("id: TASK-001"));
        assert!(body.contains("auth middleware"));
    }

    #[test]
    fn parse_front_matter_missing() {
        assert!(parse_front_matter("No front matter here").is_none());
    }

    #[test]
    fn normalize_issue_full() {
        let (fm_str, body) = parse_front_matter(SAMPLE_ISSUE).unwrap();
        let fm: IssueFrontMatter = serde_yaml::from_str(fm_str).unwrap();
        let issue = normalize_issue(&fm, body);

        assert_eq!(issue.id, "TASK-001");
        assert_eq!(issue.identifier, "TASK-001");
        assert_eq!(issue.title, "Fix the auth bug");
        assert_eq!(issue.state, "Todo");
        assert_eq!(issue.priority, Some(1));
        assert_eq!(issue.labels, vec!["bug", "auth"]);
        assert!(issue.blocked_by.is_empty());
        assert!(issue.created_at.is_some());
        assert!(issue.updated_at.is_some());
        assert!(issue.description.unwrap().contains("auth middleware"));
    }

    #[test]
    fn normalize_issue_with_blockers() {
        let (fm_str, body) = parse_front_matter(ISSUE_WITH_BLOCKERS).unwrap();
        let fm: IssueFrontMatter = serde_yaml::from_str(fm_str).unwrap();
        let issue = normalize_issue(&fm, body);

        assert_eq!(issue.blocked_by.len(), 1);
        assert_eq!(issue.blocked_by[0].id, Some("TASK-001".into()));
        assert_eq!(issue.blocked_by[0].identifier, Some("TASK-001".into()));
        assert_eq!(issue.blocked_by[0].state, Some("Todo".into()));
    }

    #[test]
    fn read_all_issues_from_directory() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "task-001.md", SAMPLE_ISSUE);
        write_issue(dir.path(), "task-002.md", ISSUE_WITH_BLOCKERS);
        write_issue(dir.path(), "task-003.md", DONE_ISSUE);
        // Non-.md file should be ignored
        write_issue(dir.path(), "notes.txt", "not an issue");

        let client = MarkdownClient::new(
            dir.path().to_path_buf(),
            vec!["Todo".into(), "In Progress".into()],
        );
        let issues = client.read_all_issues().unwrap();
        assert_eq!(issues.len(), 3);
    }

    #[test]
    fn read_skips_malformed_files() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "good.md", SAMPLE_ISSUE);
        write_issue(dir.path(), "bad.md", "---\ninvalid: yaml: :\n---\nbody");

        let client = MarkdownClient::new(dir.path().to_path_buf(), vec![]);
        let issues = client.read_all_issues().unwrap();
        // Only the valid one
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "TASK-001");
    }

    #[test]
    fn read_nonexistent_directory() {
        let client = MarkdownClient::new("/nonexistent/path".into(), vec![]);
        let err = client.read_all_issues().unwrap_err();
        assert!(matches!(err, TrackerError::MarkdownIoError(_)));
    }

    #[test]
    fn rewrite_state() {
        let updated = rewrite_state_in_front_matter(SAMPLE_ISSUE, "In Progress").unwrap();
        assert!(updated.contains("state: In Progress"));
        assert!(updated.contains("updated_at:"));
        // Body preserved
        assert!(updated.contains("auth middleware"));
    }

    #[test]
    fn write_state_updates_file() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "task-001.md", SAMPLE_ISSUE);

        let client = MarkdownClient::new(dir.path().to_path_buf(), vec![]);
        client.write_state("TASK-001", "Done").unwrap();

        let content = std::fs::read_to_string(dir.path().join("task-001.md")).unwrap();
        assert!(content.contains("state: Done"));
    }

    #[test]
    fn write_state_nonexistent_id_is_ok() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "task-001.md", SAMPLE_ISSUE);

        let client = MarkdownClient::new(dir.path().to_path_buf(), vec![]);
        // Non-matching id — should succeed silently
        let result = client.write_state("TASK-999", "Done");
        assert!(result.is_ok());
    }

    #[test]
    fn description_truncation() {
        let long_body = "x".repeat(5000);
        let content = format!("---\nid: T1\ntitle: Test\nstate: Todo\n---\n{long_body}");
        let (fm_str, body) = parse_front_matter(&content).unwrap();
        let fm: IssueFrontMatter = serde_yaml::from_str(fm_str).unwrap();
        let issue = normalize_issue(&fm, body);
        assert_eq!(
            issue.description.as_ref().map(|d| d.len()),
            Some(MAX_DESCRIPTION_LEN)
        );
    }

    #[test]
    fn labels_normalized_to_lowercase() {
        let content =
            "---\nid: T1\ntitle: Test\nstate: Todo\nlabels:\n  - BUG\n  - Feature\n---\nbody";
        let (fm_str, body) = parse_front_matter(content).unwrap();
        let fm: IssueFrontMatter = serde_yaml::from_str(fm_str).unwrap();
        let issue = normalize_issue(&fm, body);
        assert_eq!(issue.labels, vec!["bug", "feature"]);
    }

    #[tokio::test]
    async fn async_fetch_candidate_issues() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "task-001.md", SAMPLE_ISSUE);
        write_issue(dir.path(), "task-003.md", DONE_ISSUE);

        let client = MarkdownClient::new(dir.path().to_path_buf(), vec!["Todo".into()]);
        let issues = client.fetch_candidate_issues().await.unwrap();
        assert_eq!(issues.len(), 2);
    }

    #[tokio::test]
    async fn async_fetch_by_states() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "task-001.md", SAMPLE_ISSUE);
        write_issue(dir.path(), "task-003.md", DONE_ISSUE);

        let client = MarkdownClient::new(dir.path().to_path_buf(), vec!["Todo".into()]);
        let issues = client
            .fetch_issues_by_states(&["Done".into()])
            .await
            .unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "TASK-003");
    }

    #[tokio::test]
    async fn async_fetch_empty_states() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "task-001.md", SAMPLE_ISSUE);

        let client = MarkdownClient::new(dir.path().to_path_buf(), vec![]);
        let issues = client.fetch_issues_by_states(&[]).await.unwrap();
        assert!(issues.is_empty());
    }

    #[tokio::test]
    async fn async_fetch_by_ids() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "task-001.md", SAMPLE_ISSUE);
        write_issue(dir.path(), "task-002.md", ISSUE_WITH_BLOCKERS);

        let client = MarkdownClient::new(dir.path().to_path_buf(), vec![]);
        let issues = client
            .fetch_issue_states_by_ids(&["TASK-002".into()])
            .await
            .unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "TASK-002");
    }

    #[tokio::test]
    async fn async_fetch_empty_ids() {
        let dir = TempDir::new().unwrap();
        let client = MarkdownClient::new(dir.path().to_path_buf(), vec![]);
        let issues = client.fetch_issue_states_by_ids(&[]).await.unwrap();
        assert!(issues.is_empty());
    }

    #[tokio::test]
    async fn async_set_issue_state() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "task-001.md", SAMPLE_ISSUE);

        let client = MarkdownClient::new(dir.path().to_path_buf(), vec![]);
        client.set_issue_state("TASK-001", "Done").await.unwrap();

        let content = std::fs::read_to_string(dir.path().join("task-001.md")).unwrap();
        assert!(content.contains("state: Done"));
    }

    // ── Journal Tests ──

    #[test]
    fn journal_logs_state_transition() {
        let dir = TempDir::new().unwrap();
        let journal = Journal::new(dir.path(), None);

        journal
            .log_state_transition("TASK-001", "Todo", "In Progress", "Fix the bug")
            .unwrap();

        let entries = journal.read_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].payload.payload_type, "Custom");
        assert_eq!(
            entries[0].payload.event_type,
            "symphony.tracker.state_transition"
        );
        assert_eq!(entries[0].payload.data["issue_id"], "TASK-001");
        assert_eq!(entries[0].payload.data["from_state"], "Todo");
        assert_eq!(entries[0].payload.data["to_state"], "In Progress");
        assert_eq!(entries[0].payload.data["issue_title"], "Fix the bug");
    }

    #[test]
    fn journal_logs_scan() {
        let dir = TempDir::new().unwrap();
        let journal = Journal::new(dir.path(), None);

        let issues = vec![
            Issue {
                id: "T1".into(),
                identifier: "T1".into(),
                title: "First".into(),
                state: "Todo".into(),
                description: None,
                priority: None,
                branch_name: None,
                url: None,
                labels: vec![],
                blocked_by: vec![],
                created_at: None,
                updated_at: None,
            },
            Issue {
                id: "T2".into(),
                identifier: "T2".into(),
                title: "Second".into(),
                state: "Done".into(),
                description: None,
                priority: None,
                branch_name: None,
                url: None,
                labels: vec![],
                blocked_by: vec![],
                created_at: None,
                updated_at: None,
            },
        ];

        journal.log_scan(&issues).unwrap();

        let entries = journal.read_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].payload.event_type, "symphony.tracker.scan");
        assert_eq!(entries[0].payload.data["issue_count"], 2);
    }

    #[test]
    fn journal_appends_multiple_entries() {
        let dir = TempDir::new().unwrap();
        let journal = Journal::new(dir.path(), None);

        journal
            .log_state_transition("T1", "Todo", "In Progress", "A")
            .unwrap();
        journal
            .log_state_transition("T1", "In Progress", "Done", "A")
            .unwrap();
        journal
            .log_state_transition("T2", "Todo", "Done", "B")
            .unwrap();

        let entries = journal.read_entries().unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn journal_read_empty_returns_empty() {
        let dir = TempDir::new().unwrap();
        let journal = Journal::new(dir.path(), None);
        let entries = journal.read_entries().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn journal_entry_has_lago_compatible_schema() {
        let dir = TempDir::new().unwrap();
        let journal = Journal::new(dir.path(), None);

        journal
            .log_state_transition("T1", "Todo", "Done", "Test")
            .unwrap();

        // Read raw JSONL and verify schema
        let content = std::fs::read_to_string(dir.path().join(".journal.jsonl")).unwrap();
        let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();

        // Lago EventEnvelope fields
        assert!(entry.get("event_id").is_some());
        assert!(entry.get("session_id").is_some());
        assert!(entry.get("branch_id").is_some());
        assert!(entry.get("timestamp").is_some());
        assert!(entry.get("metadata").is_some());

        // Lago EventPayload::Custom structure
        let payload = entry.get("payload").unwrap();
        assert_eq!(payload["type"], "Custom");
        assert!(payload.get("event_type").is_some());
        assert!(payload.get("data").is_some());
    }

    #[test]
    fn write_state_with_journal_creates_entries() {
        let dir = TempDir::new().unwrap();
        write_issue(dir.path(), "task-001.md", SAMPLE_ISSUE);

        let client = MarkdownClient::with_journal(dir.path().to_path_buf(), vec![], None);
        client.write_state("TASK-001", "Done").unwrap();

        let content = std::fs::read_to_string(dir.path().join("task-001.md")).unwrap();
        assert!(content.contains("state: Done"));

        // Journal should have the transition
        let journal = Journal::new(dir.path(), None);
        let entries = journal.read_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].payload.data["from_state"], "Todo");
        assert_eq!(entries[0].payload.data["to_state"], "Done");
    }

    #[tokio::test]
    async fn lago_check_returns_none_when_no_endpoint() {
        let dir = TempDir::new().unwrap();
        let journal = Journal::new(dir.path(), None);
        assert!(journal.check_lago().await.is_none());
    }

    #[tokio::test]
    async fn lago_check_returns_none_for_unreachable() {
        let dir = TempDir::new().unwrap();
        let journal = Journal::new(dir.path(), Some("http://localhost:19999".into()));
        assert!(journal.check_lago().await.is_none());
    }
}
