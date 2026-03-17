// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Workspace types (Spec Section 4.1.4).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Filesystem workspace assigned to one issue identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub path: PathBuf,
    pub workspace_key: String,
    /// Whether the directory was newly created in this call.
    pub created_now: bool,
}
