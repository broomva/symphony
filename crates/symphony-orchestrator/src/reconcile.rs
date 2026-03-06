//! Reconciliation logic (Spec Section 8.5).

use symphony_core::OrchestratorState;

/// Calculate retry backoff delay (Spec Section 8.4).
///
/// Normal continuation: 1000ms fixed.
/// Failure-driven: min(10000 * 2^(attempt-1), max_backoff_ms).
pub fn backoff_delay_ms(attempt: u32, max_backoff_ms: u64, is_continuation: bool) -> u64 {
    if is_continuation {
        return 1000;
    }
    let base: u64 = 10_000;
    let power = attempt.saturating_sub(1);
    let delay = base.saturating_mul(1u64 << power.min(20));
    delay.min(max_backoff_ms)
}

/// Check if an issue state is terminal.
pub fn is_terminal_state(state: &str, terminal_states: &[String]) -> bool {
    let normalized = state.trim().to_lowercase();
    terminal_states
        .iter()
        .any(|s| s.trim().to_lowercase() == normalized)
}

/// Check if an issue state is active.
pub fn is_active_state(state: &str, active_states: &[String]) -> bool {
    let normalized = state.trim().to_lowercase();
    active_states
        .iter()
        .any(|s| s.trim().to_lowercase() == normalized)
}

/// Check for stalled sessions (Spec Section 8.5 Part A).
pub fn find_stalled_issues(
    state: &OrchestratorState,
    stall_timeout_ms: i64,
    now_ms: i64,
) -> Vec<String> {
    if stall_timeout_ms <= 0 {
        return vec![];
    }

    state
        .running
        .iter()
        .filter(|(_, entry)| {
            let last_activity = entry
                .last_codex_timestamp
                .map(|t| t.timestamp_millis())
                .unwrap_or(entry.started_at.timestamp_millis());
            let elapsed = now_ms - last_activity;
            elapsed > stall_timeout_ms
        })
        .map(|(id, _)| id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_backoff_is_1s() {
        assert_eq!(backoff_delay_ms(1, 300_000, true), 1000);
        assert_eq!(backoff_delay_ms(5, 300_000, true), 1000);
    }

    #[test]
    fn failure_backoff_scales_exponentially() {
        assert_eq!(backoff_delay_ms(1, 300_000, false), 10_000);
        assert_eq!(backoff_delay_ms(2, 300_000, false), 20_000);
        assert_eq!(backoff_delay_ms(3, 300_000, false), 40_000);
    }

    #[test]
    fn failure_backoff_capped() {
        assert_eq!(backoff_delay_ms(10, 300_000, false), 300_000);
    }

    #[test]
    fn terminal_state_check() {
        let terminals = vec!["Done".into(), "Closed".into()];
        assert!(is_terminal_state("Done", &terminals));
        assert!(is_terminal_state("  done  ", &terminals));
        assert!(!is_terminal_state("In Progress", &terminals));
    }

    #[test]
    fn active_state_check() {
        let actives = vec!["Todo".into(), "In Progress".into()];
        assert!(is_active_state("Todo", &actives));
        assert!(is_active_state("in progress", &actives));
        assert!(!is_active_state("Done", &actives));
    }
}
