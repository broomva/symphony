// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! EGRI evaluation integration for Symphony orchestrator.
//!
//! Mode 1 (Batch): Periodic evaluation alongside the poll loop.
//! Mode 2 (Hive): Wire autoany-core's EgriLoop into HiveCoordinator (feature-gated).

pub mod batch;
pub mod config;
#[cfg(feature = "hive")]
pub mod hive_adapter;
pub mod journal;
pub mod types;

pub use batch::BatchEgriRunner;
pub use config::EgriState;
pub use types::{EvalRecord, EvalSnapshot};
