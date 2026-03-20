// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Mode 2: Hive EGRI adapter — wire autoany-core traits into HiveCoordinator.
//!
//! This module implements autoany-core's Proposer, Executor, Evaluator, and Selector
//! traits for Symphony's hive multi-agent evolution mode.
//!
//! Gated behind the `hive` feature flag.

// TODO(Mode 2): Implement trait adapters for autoany-core:
//   - Proposer<HiveArtifact>: new agent session with varied prompt
//   - Executor<HiveArtifact>: run_worker() via subprocess or Arcan
//   - Evaluator<HiveArtifact>: eval_script or state-based resolution rate
//   - Selector: KeepIfImproves / HumanGate based on autonomy config
