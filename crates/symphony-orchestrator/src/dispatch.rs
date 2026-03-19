// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Dispatch logic (Spec Section 8.2, 8.3).
//!
//! Candidate selection, sorting, eligibility checking, concurrency control.

use std::collections::HashMap;

use symphony_config::types::HiveConfig;
use symphony_core::{Issue, OrchestratorState};

/// Check if an issue is eligible for dispatch (Spec Section 8.2).
pub fn is_dispatch_eligible(
    issue: &Issue,
    state: &OrchestratorState,
    terminal_states: &[String],
    active_states: &[String],
    per_state_limits: &HashMap<String, u32>,
) -> bool {
    // Must have required fields (S8.2)
    if issue.id.is_empty()
        || issue.identifier.is_empty()
        || issue.title.is_empty()
        || issue.state.is_empty()
    {
        return false;
    }

    let normalized_state = issue.state.trim().to_lowercase();

    // Must be in active states (S8.2)
    if !active_states
        .iter()
        .any(|s| s.trim().to_lowercase() == normalized_state)
    {
        return false;
    }

    // Must not be in terminal states (S8.2)
    if terminal_states
        .iter()
        .any(|s| s.trim().to_lowercase() == normalized_state)
    {
        return false;
    }

    // Must not be already running or claimed (S8.2)
    if state.running.contains_key(&issue.id) || state.is_claimed(&issue.id) {
        return false;
    }

    // Must have global slots available (S8.3)
    if state.available_slots() == 0 {
        return false;
    }

    // Per-state concurrency check (S8.3)
    if let Some(&limit) = per_state_limits.get(&normalized_state) {
        let running_in_state = state
            .running
            .values()
            .filter(|r| r.issue.state.trim().to_lowercase() == normalized_state)
            .count() as u32;
        if running_in_state >= limit {
            return false;
        }
    }

    // Blocker rule: Todo issues with non-terminal blockers are not eligible (S8.2)
    if normalized_state == "todo" {
        let has_non_terminal_blocker = issue.blocked_by.iter().any(|b| {
            b.state
                .as_ref()
                .map(|s| {
                    !terminal_states
                        .iter()
                        .any(|ts| ts.trim().to_lowercase() == s.trim().to_lowercase())
                })
                .unwrap_or(true) // unknown state = non-terminal
        });
        if has_non_terminal_blocker {
            return false;
        }
    }

    true
}

/// Sort issues for dispatch priority (Spec Section 8.2).
///
/// 1. priority ascending (1..4 preferred; null sorts last)
/// 2. created_at oldest first
/// 3. identifier lexicographic
pub fn sort_for_dispatch(issues: &mut [Issue]) {
    issues.sort_by(|a, b| {
        let pa = a.priority.unwrap_or(i32::MAX);
        let pb = b.priority.unwrap_or(i32::MAX);
        pa.cmp(&pb)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.identifier.cmp(&b.identifier))
    });
}

/// Count running issues in a specific state.
pub fn running_in_state(state: &OrchestratorState, normalized_state: &str) -> u32 {
    state
        .running
        .values()
        .filter(|r| r.issue.state.trim().to_lowercase() == normalized_state)
        .count() as u32
}

/// Check per-state concurrency availability (S8.3).
pub fn has_per_state_slot(
    state: &OrchestratorState,
    issue_state: &str,
    per_state_limits: &HashMap<String, u32>,
) -> bool {
    let normalized = issue_state.trim().to_lowercase();
    match per_state_limits.get(&normalized) {
        Some(&limit) => running_in_state(state, &normalized) < limit,
        None => true, // no per-state limit → use global only
    }
}

/// Check if an issue is eligible for hive multi-agent dispatch.
///
/// Same rules as `is_dispatch_eligible` EXCEPT:
/// - Instead of blocking on `running.contains_key(&issue.id)`,
///   counts agents as `running_for_issue = keys starting with "{issue.id}:hive-"`.
/// - Blocks only when `running_for_issue >= hive_config.agents_per_task`.
pub fn is_hive_dispatch_eligible(
    issue: &Issue,
    state: &OrchestratorState,
    terminal_states: &[String],
    active_states: &[String],
    per_state_limits: &HashMap<String, u32>,
    hive_config: &HiveConfig,
) -> bool {
    // Must have required fields
    if issue.id.is_empty()
        || issue.identifier.is_empty()
        || issue.title.is_empty()
        || issue.state.is_empty()
    {
        return false;
    }

    let normalized_state = issue.state.trim().to_lowercase();

    // Must be in active states
    if !active_states
        .iter()
        .any(|s| s.trim().to_lowercase() == normalized_state)
    {
        return false;
    }

    // Must not be in terminal states
    if terminal_states
        .iter()
        .any(|s| s.trim().to_lowercase() == normalized_state)
    {
        return false;
    }

    // Must not be claimed (retry queue)
    if state.is_claimed(&issue.id) {
        return false;
    }

    // Hive-specific: count agents running for this issue
    let hive_prefix = format!("{}:hive-", issue.id);
    let running_for_issue = state
        .running
        .keys()
        .filter(|k| k.starts_with(&hive_prefix))
        .count() as u32;

    if running_for_issue >= hive_config.agents_per_task {
        return false;
    }

    // Must have global slots available
    if state.available_slots() == 0 {
        return false;
    }

    // Per-state concurrency check
    if let Some(&limit) = per_state_limits.get(&normalized_state) {
        let running_in = running_in_state(state, &normalized_state);
        if running_in >= limit {
            return false;
        }
    }

    true
}

/// Check if an issue should use hive dispatch.
pub fn is_hive_issue(issue: &Issue, hive_config: &HiveConfig) -> bool {
    hive_config.enabled && issue.labels.iter().any(|l| l == "hive")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use symphony_core::{BlockerRef, Issue};

    fn make_issue(id: &str, identifier: &str, priority: Option<i32>, state: &str) -> Issue {
        Issue {
            id: id.into(),
            identifier: identifier.into(),
            title: "Test".into(),
            description: None,
            priority,
            state: state.into(),
            branch_name: None,
            url: None,
            labels: vec![],
            blocked_by: vec![],
            created_at: Some(Utc::now()),
            updated_at: None,
        }
    }

    #[test]
    fn sort_by_priority() {
        let mut issues = vec![
            make_issue("3", "C-3", Some(3), "Todo"),
            make_issue("1", "A-1", Some(1), "Todo"),
            make_issue("2", "B-2", Some(2), "Todo"),
        ];
        sort_for_dispatch(&mut issues);
        assert_eq!(issues[0].identifier, "A-1");
        assert_eq!(issues[1].identifier, "B-2");
        assert_eq!(issues[2].identifier, "C-3");
    }

    #[test]
    fn null_priority_sorts_last() {
        let mut issues = vec![
            make_issue("1", "A-1", None, "Todo"),
            make_issue("2", "B-2", Some(1), "Todo"),
        ];
        sort_for_dispatch(&mut issues);
        assert_eq!(issues[0].identifier, "B-2");
        assert_eq!(issues[1].identifier, "A-1");
    }

    #[test]
    fn same_priority_sorted_by_created_at() {
        use chrono::Duration;
        let now = Utc::now();
        let mut issue_b = make_issue("2", "B-2", Some(1), "Todo");
        issue_b.created_at = Some(now);
        let mut issue_a = make_issue("1", "A-1", Some(1), "Todo");
        issue_a.created_at = Some(now - Duration::hours(1)); // A is older

        let mut issues = vec![issue_b, issue_a];
        sort_for_dispatch(&mut issues);
        // Oldest first
        assert_eq!(issues[0].identifier, "A-1");
        assert_eq!(issues[1].identifier, "B-2");
    }

    #[test]
    fn eligible_basic() {
        let state = OrchestratorState::new(30000, 10);
        let issue = make_issue("1", "T-1", Some(1), "Todo");
        let active = vec!["Todo".into()];
        let terminal = vec!["Done".into()];
        assert!(is_dispatch_eligible(
            &issue,
            &state,
            &terminal,
            &active,
            &HashMap::new()
        ));
    }

    #[test]
    fn ineligible_when_claimed() {
        let mut state = OrchestratorState::new(30000, 10);
        state.claimed.insert("1".into());
        let issue = make_issue("1", "T-1", Some(1), "Todo");
        assert!(!is_dispatch_eligible(
            &issue,
            &state,
            &["Done".into()],
            &["Todo".into()],
            &HashMap::new()
        ));
    }

    #[test]
    fn ineligible_missing_title() {
        let state = OrchestratorState::new(30000, 10);
        let mut issue = make_issue("1", "T-1", Some(1), "Todo");
        issue.title = String::new();
        assert!(!is_dispatch_eligible(
            &issue,
            &state,
            &["Done".into()],
            &["Todo".into()],
            &HashMap::new()
        ));
    }

    #[test]
    fn ineligible_not_active_state() {
        let state = OrchestratorState::new(30000, 10);
        let issue = make_issue("1", "T-1", Some(1), "Backlog");
        assert!(!is_dispatch_eligible(
            &issue,
            &state,
            &["Done".into()],
            &["Todo".into()],
            &HashMap::new()
        ));
    }

    #[test]
    fn todo_with_non_terminal_blocker_ineligible() {
        let state = OrchestratorState::new(30000, 10);
        let mut issue = make_issue("1", "T-1", Some(1), "Todo");
        issue.blocked_by.push(BlockerRef {
            id: Some("b1".into()),
            identifier: Some("BLOCK-1".into()),
            state: Some("In Progress".into()), // non-terminal
        });
        assert!(!is_dispatch_eligible(
            &issue,
            &state,
            &["Done".into()],
            &["Todo".into()],
            &HashMap::new()
        ));
    }

    #[test]
    fn todo_with_all_terminal_blockers_eligible() {
        let state = OrchestratorState::new(30000, 10);
        let mut issue = make_issue("1", "T-1", Some(1), "Todo");
        issue.blocked_by.push(BlockerRef {
            id: Some("b1".into()),
            identifier: Some("BLOCK-1".into()),
            state: Some("Done".into()), // terminal
        });
        assert!(is_dispatch_eligible(
            &issue,
            &state,
            &["Done".into()],
            &["Todo".into()],
            &HashMap::new()
        ));
    }

    #[test]
    fn global_concurrency_limit() {
        let mut state = OrchestratorState::new(30000, 2);
        // Fill up both slots
        for i in 0..2 {
            let id = format!("running-{i}");
            state.running.insert(
                id.clone(),
                symphony_core::state::RunningEntry {
                    identifier: format!("R-{i}"),
                    issue: make_issue(&id, &format!("R-{i}"), Some(1), "Todo"),
                    session_id: None,
                    codex_app_server_pid: None,
                    last_codex_message: None,
                    last_codex_event: None,
                    last_codex_timestamp: None,
                    codex_input_tokens: 0,
                    codex_output_tokens: 0,
                    codex_total_tokens: 0,
                    last_reported_input_tokens: 0,
                    last_reported_output_tokens: 0,
                    last_reported_total_tokens: 0,
                    retry_attempt: None,
                    started_at: Utc::now(),
                    turn_count: 0,
                },
            );
        }
        let issue = make_issue("new", "N-1", Some(1), "Todo");
        assert!(!is_dispatch_eligible(
            &issue,
            &state,
            &["Done".into()],
            &["Todo".into()],
            &HashMap::new()
        ));
    }

    #[test]
    fn hive_issue_detection() {
        let config = HiveConfig {
            enabled: true,
            ..Default::default()
        };
        let mut issue = make_issue("1", "T-1", Some(1), "Todo");
        issue.labels = vec!["hive".into()];
        assert!(is_hive_issue(&issue, &config));

        issue.labels = vec!["normal".into()];
        assert!(!is_hive_issue(&issue, &config));

        let disabled_config = HiveConfig::default();
        issue.labels = vec!["hive".into()];
        assert!(!is_hive_issue(&issue, &disabled_config));
    }

    #[test]
    fn hive_dispatch_allows_multiple_agents() {
        let state = OrchestratorState::new(30000, 10);
        let mut issue = make_issue("1", "T-1", Some(1), "Todo");
        issue.labels = vec!["hive".into()];
        let config = HiveConfig {
            enabled: true,
            agents_per_task: 3,
            ..Default::default()
        };

        // Should be eligible — no agents running yet
        assert!(is_hive_dispatch_eligible(
            &issue,
            &state,
            &["Done".into()],
            &["Todo".into()],
            &HashMap::new(),
            &config,
        ));
    }

    #[test]
    fn hive_dispatch_blocks_at_limit() {
        let mut state = OrchestratorState::new(30000, 10);
        let config = HiveConfig {
            enabled: true,
            agents_per_task: 2,
            ..Default::default()
        };

        // Add 2 running hive agents for issue "1"
        for i in 0..2 {
            let key = format!("1:hive-{i}");
            state.running.insert(
                key.clone(),
                symphony_core::state::RunningEntry {
                    identifier: format!("T-1:hive-{i}"),
                    issue: make_issue("1", "T-1", Some(1), "Todo"),
                    session_id: None,
                    codex_app_server_pid: None,
                    last_codex_message: None,
                    last_codex_event: None,
                    last_codex_timestamp: None,
                    codex_input_tokens: 0,
                    codex_output_tokens: 0,
                    codex_total_tokens: 0,
                    last_reported_input_tokens: 0,
                    last_reported_output_tokens: 0,
                    last_reported_total_tokens: 0,
                    retry_attempt: None,
                    started_at: Utc::now(),
                    turn_count: 0,
                },
            );
        }

        let issue = make_issue("1", "T-1", Some(1), "Todo");
        assert!(!is_hive_dispatch_eligible(
            &issue,
            &state,
            &["Done".into()],
            &["Todo".into()],
            &HashMap::new(),
            &config,
        ));
    }

    #[test]
    fn per_state_concurrency_limit() {
        let mut state = OrchestratorState::new(30000, 10);
        // Add one running "todo" issue
        state.running.insert(
            "running-1".into(),
            symphony_core::state::RunningEntry {
                identifier: "R-1".into(),
                issue: make_issue("running-1", "R-1", Some(1), "Todo"),
                session_id: None,
                codex_app_server_pid: None,
                last_codex_message: None,
                last_codex_event: None,
                last_codex_timestamp: None,
                codex_input_tokens: 0,
                codex_output_tokens: 0,
                codex_total_tokens: 0,
                last_reported_input_tokens: 0,
                last_reported_output_tokens: 0,
                last_reported_total_tokens: 0,
                retry_attempt: None,
                started_at: Utc::now(),
                turn_count: 0,
            },
        );
        let issue = make_issue("new", "N-1", Some(1), "Todo");
        let mut per_state = HashMap::new();
        per_state.insert("todo".into(), 1); // limit 1 for todo

        assert!(!is_dispatch_eligible(
            &issue,
            &state,
            &["Done".into()],
            &["Todo".into()],
            &per_state
        ));
    }
}
