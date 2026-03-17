//! Symphony configuration layer (Spec Sections 5, 6, 12).
//!
//! Loads WORKFLOW.md, parses YAML front matter + prompt body,
//! provides typed getters with defaults and env resolution,
//! and renders prompt templates via Liquid.

pub mod loader;
pub mod template;
pub mod types;
pub mod watcher;

pub use loader::load_workflow;
pub use template::{TemplateError, render_prompt};
pub use types::{
    AgentConfig, CodexConfig, HooksConfig, PollingConfig, ServiceConfig, TrackerConfig,
    WorkflowDefinition, WorkspaceConfig,
};
