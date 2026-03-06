//! Symphony configuration layer (Spec Sections 5, 6).
//!
//! Loads WORKFLOW.md, parses YAML front matter + prompt body,
//! and provides typed getters with defaults and env resolution.

pub mod loader;
pub mod types;
pub mod watcher;

pub use loader::load_workflow;
pub use types::{
    AgentConfig, CodexConfig, HooksConfig, PollingConfig, ServiceConfig, TrackerConfig,
    WorkflowDefinition, WorkspaceConfig,
};
