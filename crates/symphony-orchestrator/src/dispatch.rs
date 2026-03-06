//! Dispatch logic (Spec Section 8.2).
//!
//! Candidate selection, sorting, eligibility checking.

use symphony_core::{Issue, OrchestratorState};

/// Check if an issue is eligible for dispatch (Spec Section 8.2).
pub fn is_dispatch_eligible(
    issue: &Issue,
    state: &OrchestratorState,
    terminal_states: &[String],
    active_states: &[String],
    per_state_limits: &std::collections::HashMap<String, u32>,
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

    // Must not be already running or claimed
    if state.running.contains_key(&issue.id) || state.is_claimed(&issue.id) {
        return false;
    }

    // Must have global slots available
    if state.available_slots() == 0 {
        return false;
    }

    // Per-state concurrency check
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

    // Blocker rule: Todo issues with non-terminal blockers are not eligible
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
        // Priority: lower is better, None sorts last
        let pa = a.priority.unwrap_or(i32::MAX);
        let pb = b.priority.unwrap_or(i32::MAX);
        pa.cmp(&pb)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.identifier.cmp(&b.identifier))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use symphony_core::Issue;

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
            &std::collections::HashMap::new()
        ));
    }

    #[test]
    fn ineligible_when_claimed() {
        let mut state = OrchestratorState::new(30000, 10);
        state.claimed.insert("1".into());
        let issue = make_issue("1", "T-1", Some(1), "Todo");
        let active = vec!["Todo".into()];
        let terminal = vec!["Done".into()];
        assert!(!is_dispatch_eligible(
            &issue,
            &state,
            &terminal,
            &active,
            &std::collections::HashMap::new()
        ));
    }
}
