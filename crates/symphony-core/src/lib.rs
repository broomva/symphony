//! Symphony core domain model.
//!
//! Defines the shared types used across all Symphony layers:
//! Issue, WorkflowDefinition, ServiceConfig, Workspace, RunAttempt,
//! LiveSession, RetryEntry, and OrchestratorState.

pub mod issue;
pub mod session;
pub mod state;
pub mod workspace;

pub use issue::{BlockerRef, Issue};
pub use session::{LiveSession, RetryEntry, RunAttempt, RunAttemptStatus};
pub use state::OrchestratorState;
pub use workspace::Workspace;
