// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! EGRI runtime state tracking.

use crate::types::EvalSnapshot;

/// Runtime state for EGRI evaluation (shared with observability server).
#[derive(Debug, Clone, Default)]
pub struct EgriState {
    pub snapshot: EvalSnapshot,
}
