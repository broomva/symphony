//! Agent runner protocol (Spec Section 10).
//!
//! Wraps workspace + prompt + app-server client.
//! Launches the coding agent subprocess with JSON line protocol.

pub mod protocol;
pub mod runner;

pub use runner::{AgentRunner, LinearToolConfig};
