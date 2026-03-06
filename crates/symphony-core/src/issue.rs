//! Normalized issue record (Spec Section 4.1.1).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A normalized issue from the tracker, used for orchestration, prompt rendering,
/// and observability output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    /// Stable tracker-internal ID.
    pub id: String,
    /// Human-readable ticket key (e.g. `ABC-123`).
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    /// Lower numbers are higher priority in dispatch sorting.
    pub priority: Option<i32>,
    /// Current tracker state name.
    pub state: String,
    /// Tracker-provided branch metadata if available.
    pub branch_name: Option<String>,
    pub url: Option<String>,
    /// Normalized to lowercase.
    pub labels: Vec<String>,
    pub blocked_by: Vec<BlockerRef>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// A reference to a blocking issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockerRef {
    pub id: Option<String>,
    pub identifier: Option<String>,
    pub state: Option<String>,
}

impl Issue {
    /// Sanitize the identifier to a workspace-safe key.
    /// Replace any character not in `[A-Za-z0-9._-]` with `_`.
    pub fn workspace_key(&self) -> String {
        self.identifier
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_key_sanitizes_identifier() {
        let issue = Issue {
            id: "id1".into(),
            identifier: "ABC-123".into(),
            title: "Test".into(),
            description: None,
            priority: Some(1),
            state: "Todo".into(),
            branch_name: None,
            url: None,
            labels: vec![],
            blocked_by: vec![],
            created_at: None,
            updated_at: None,
        };
        assert_eq!(issue.workspace_key(), "ABC-123");
    }

    #[test]
    fn workspace_key_replaces_special_chars() {
        let issue = Issue {
            id: "id2".into(),
            identifier: "PROJ/feat#42".into(),
            title: "Test".into(),
            description: None,
            priority: None,
            state: "Todo".into(),
            branch_name: None,
            url: None,
            labels: vec![],
            blocked_by: vec![],
            created_at: None,
            updated_at: None,
        };
        assert_eq!(issue.workspace_key(), "PROJ_feat_42");
    }
}
