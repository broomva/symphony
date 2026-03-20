// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Hive coordinator — manages multi-agent collaborative evolution for an issue.
//!
//! When an issue has the `hive` label and `hive.enabled: true` in config,
//! Symphony dispatches N concurrent agents per generation, each running EGRI loops.
//! After all agents complete, the coordinator selects the generation winner,
//! checks convergence, and either starts the next generation or completes.

use std::path::Path;

use symphony_config::types::HiveConfig;
use symphony_core::Issue;

/// Result of a single hive generation.
#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub generation: u32,
    pub best_score: f32,
    pub best_session_id: String,
    pub agent_scores: Vec<(String, f32)>,
}

/// Final result of a completed hive task.
#[derive(Debug, Clone)]
pub struct HiveResult {
    pub hive_task_id: String,
    pub total_generations: u32,
    pub total_trials: u32,
    pub final_score: f32,
    pub winning_session_id: String,
}

/// Session context for a hive agent.
#[derive(Debug, Clone)]
pub struct HiveAgentSession {
    pub session_id: String,
    pub agent_index: u32,
    pub generation: u32,
    pub score: Option<f32>,
    pub artifact_summary: Option<String>,
}

/// Orchestrates the hive collaborative evolution loop.
///
/// For each generation:
/// 1. Start N agents in parallel, each running EGRI loops
/// 2. Collect scored artifacts from all agents
/// 3. Select the generation winner
/// 4. Check convergence → next generation or complete
pub struct HiveCoordinator {
    pub hive_task_id: String,
    pub issue: Issue,
    pub config: HiveConfig,
    pub agent_sessions: Vec<HiveAgentSession>,
    pub current_generation: u32,
    pub best_global_score: f32,
    previous_best_score: f32,
}

impl HiveCoordinator {
    pub fn new(hive_task_id: String, issue: Issue, config: HiveConfig) -> Self {
        Self {
            hive_task_id,
            issue,
            config,
            agent_sessions: Vec::new(),
            current_generation: 0,
            best_global_score: 0.0,
            previous_best_score: 0.0,
        }
    }

    /// Check if the hive loop should continue to the next generation.
    pub fn should_continue(&self) -> bool {
        if self.current_generation >= self.config.max_generations {
            return false;
        }
        if self.current_generation > 0 {
            let improvement = (self.best_global_score - self.previous_best_score).abs() as f64;
            if improvement < self.config.convergence_threshold {
                return false;
            }
        }
        true
    }

    /// Record a generation result and advance state.
    pub fn complete_generation(&mut self, result: &GenerationResult) {
        self.previous_best_score = self.best_global_score;
        self.best_global_score = result.best_score;
        self.current_generation = result.generation;
    }

    /// Build the prompt context prefix for a hive agent.
    #[allow(clippy::too_many_arguments)]
    pub fn build_hive_prompt(
        &self,
        agent_index: u32,
        total_agents: u32,
        generation: u32,
        previous_winner_artifact: Option<&str>,
        previous_score: Option<f32>,
        peer_summaries: &[String],
        original_prompt: &str,
    ) -> String {
        let mut prompt = String::new();
        prompt.push_str("## Hive Context\n");
        prompt.push_str(&format!(
            "You are agent {agent_index} of {total_agents} working on this task. Generation: {generation}.\n"
        ));

        if let Some(artifact) = previous_winner_artifact {
            let score = previous_score.unwrap_or(0.0);
            prompt.push_str(&format!("### Previous Best (score: {score:.3})\n"));
            prompt.push_str(artifact);
            prompt.push('\n');
        }

        if !peer_summaries.is_empty() {
            prompt.push_str("### Peer Approaches\n");
            for summary in peer_summaries {
                prompt.push_str(&format!("- {summary}\n"));
            }
        }

        prompt.push_str("### Directive\n");
        prompt.push_str("Build on the previous best. Try a different approach from peers.\n---\n");
        prompt.push_str(original_prompt);

        prompt
    }

    /// Generate the session ID for a hive agent.
    pub fn session_id(&self, generation: u32, agent_index: u32) -> String {
        format!(
            "hive-{}-gen{}-agent{}",
            self.hive_task_id, generation, agent_index
        )
    }

    /// Generate the running map key for a hive agent.
    pub fn running_key(&self, agent_index: u32) -> String {
        format!("{}:hive-{}", self.issue.id, agent_index)
    }

    /// Select the best agent from generation results.
    pub fn select_winner(results: &[(String, f32)]) -> Option<(String, f32)> {
        results
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .cloned()
    }

    /// Return a summary of the hive run (for use in the workspace).
    pub fn summary_path(workspace: &Path, hive_task_id: &str) -> std::path::PathBuf {
        workspace.join(format!(".hive-{hive_task_id}-summary.md"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_issue() -> Issue {
        Issue {
            id: "I1".into(),
            identifier: "T-1".into(),
            title: "Test Hive".into(),
            description: Some("Test hive task".into()),
            priority: Some(1),
            state: "Todo".into(),
            branch_name: None,
            url: None,
            labels: vec!["hive".into()],
            blocked_by: vec![],
            created_at: Some(Utc::now()),
            updated_at: None,
        }
    }

    fn make_config() -> HiveConfig {
        HiveConfig {
            enabled: true,
            agents_per_task: 3,
            max_generations: 5,
            convergence_threshold: 0.01,
            egri_budget_per_agent: 10,
            eval_script: None,
            spaces_server_id: None,
            agent_profiles: vec![],
        }
    }

    #[test]
    fn should_continue_respects_max_generations() {
        let mut coord = HiveCoordinator::new("H1".into(), make_issue(), make_config());
        coord.current_generation = 5;
        assert!(!coord.should_continue());
    }

    #[test]
    fn should_continue_detects_convergence() {
        let mut coord = HiveCoordinator::new("H1".into(), make_issue(), make_config());
        coord.current_generation = 2;
        coord.best_global_score = 0.95;
        coord.previous_best_score = 0.949; // improvement < 0.01
        assert!(!coord.should_continue());
    }

    #[test]
    fn should_continue_allows_next_generation() {
        let mut coord = HiveCoordinator::new("H1".into(), make_issue(), make_config());
        coord.current_generation = 2;
        coord.best_global_score = 0.95;
        coord.previous_best_score = 0.80; // improvement > 0.01
        assert!(coord.should_continue());
    }

    #[test]
    fn select_winner_picks_highest_score() {
        let results = vec![("S1".into(), 0.8), ("S2".into(), 0.95), ("S3".into(), 0.7)];
        let (id, score) = HiveCoordinator::select_winner(&results).unwrap();
        assert_eq!(id, "S2");
        assert_eq!(score, 0.95);
    }

    #[test]
    fn session_id_format() {
        let coord = HiveCoordinator::new("H1".into(), make_issue(), make_config());
        assert_eq!(coord.session_id(2, 1), "hive-H1-gen2-agent1");
    }

    #[test]
    fn running_key_format() {
        let coord = HiveCoordinator::new("H1".into(), make_issue(), make_config());
        assert_eq!(coord.running_key(0), "I1:hive-0");
    }

    #[test]
    fn build_hive_prompt_includes_context() {
        let coord = HiveCoordinator::new("H1".into(), make_issue(), make_config());
        let prompt = coord.build_hive_prompt(
            1,
            3,
            2,
            Some("def solve(): return 42"),
            Some(0.87),
            &[
                "tried brute force".into(),
                "tried dynamic programming".into(),
            ],
            "Fix the sorting bug in sort.py",
        );

        assert!(prompt.contains("agent 1 of 3"));
        assert!(prompt.contains("Generation: 2"));
        assert!(prompt.contains("score: 0.870"));
        assert!(prompt.contains("def solve(): return 42"));
        assert!(prompt.contains("tried brute force"));
        assert!(prompt.contains("Fix the sorting bug"));
    }

    #[test]
    fn complete_generation_advances_state() {
        let mut coord = HiveCoordinator::new("H1".into(), make_issue(), make_config());
        let result = GenerationResult {
            generation: 1,
            best_score: 0.85,
            best_session_id: "S1".into(),
            agent_scores: vec![("S1".into(), 0.85), ("S2".into(), 0.7)],
        };
        coord.complete_generation(&result);
        assert_eq!(coord.current_generation, 1);
        assert_eq!(coord.best_global_score, 0.85);
    }
}
