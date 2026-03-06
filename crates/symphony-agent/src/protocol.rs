//! App-server JSON-RPC protocol messages (Spec Section 10.2).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A JSON-RPC-like protocol message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

/// Events emitted upstream to the orchestrator (Spec Section 10.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEvent {
    SessionStarted {
        session_id: String,
        thread_id: String,
        turn_id: String,
        pid: Option<String>,
    },
    StartupFailed {
        error: String,
    },
    TurnCompleted {
        usage: Option<TokenUsage>,
    },
    TurnFailed {
        error: String,
        usage: Option<TokenUsage>,
    },
    TurnCancelled {
        usage: Option<TokenUsage>,
    },
    TurnInputRequired,
    ApprovalAutoApproved {
        id: String,
    },
    UnsupportedToolCall {
        id: String,
        name: String,
    },
    Notification {
        message: String,
    },
    OtherMessage {
        method: Option<String>,
        payload: Value,
    },
    Malformed {
        raw: String,
    },
}

/// Token usage snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}
