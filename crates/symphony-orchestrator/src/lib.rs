//! Symphony orchestrator (Spec Sections 7, 8).
//!
//! Owns the poll tick, in-memory runtime state, and dispatch/retry/reconciliation logic.

pub mod dispatch;
pub mod reconcile;
pub mod scheduler;

pub use scheduler::{Scheduler, run_worker};
