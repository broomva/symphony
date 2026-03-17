// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

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

impl ProtocolMessage {
    /// Create a request message.
    pub fn request(id: u64, method: &str, params: Value) -> Self {
        Self {
            id: Some(Value::Number(id.into())),
            method: Some(method.to_string()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Create a notification (no id).
    pub fn notification(method: &str, params: Value) -> Self {
        Self {
            id: None,
            method: Some(method.to_string()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Check if this is a response (has result or error, no method).
    pub fn is_response(&self) -> bool {
        self.method.is_none() && (self.result.is_some() || self.error.is_some())
    }
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

/// Token usage snapshot (S13.5).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// Turn completion status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnOutcome {
    Completed,
    Failed(String),
    Cancelled,
    InputRequired,
    Timeout,
    ProcessExit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_message_format() {
        let msg = ProtocolMessage::request(
            1,
            "initialize",
            serde_json::json!({"clientInfo": {"name": "symphony"}}),
        );
        assert_eq!(msg.method.as_deref(), Some("initialize"));
        assert!(msg.id.is_some());
        assert!(!msg.is_response());
    }

    #[test]
    fn notification_has_no_id() {
        let msg = ProtocolMessage::notification("initialized", serde_json::json!({}));
        assert!(msg.id.is_none());
        assert_eq!(msg.method.as_deref(), Some("initialized"));
    }

    #[test]
    fn response_detection() {
        let msg = ProtocolMessage {
            id: Some(Value::Number(1.into())),
            method: None,
            params: None,
            result: Some(serde_json::json!({"threadId": "t-1"})),
            error: None,
        };
        assert!(msg.is_response());
    }

    #[test]
    fn serialize_roundtrip() {
        let msg = ProtocolMessage::request(1, "test", serde_json::json!({"key": "value"}));
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.method, msg.method);
    }

    #[test]
    fn token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn agent_event_variants() {
        // Just verify all variants can be constructed
        let _e1 = AgentEvent::SessionStarted {
            session_id: "s-1".into(),
            thread_id: "t-1".into(),
            turn_id: "u-1".into(),
            pid: Some("12345".into()),
        };
        let _e2 = AgentEvent::StartupFailed {
            error: "timeout".into(),
        };
        let _e3 = AgentEvent::TurnCompleted { usage: None };
        let _e4 = AgentEvent::TurnFailed {
            error: "err".into(),
            usage: None,
        };
        let _e5 = AgentEvent::TurnCancelled { usage: None };
        let _e6 = AgentEvent::TurnInputRequired;
        let _e7 = AgentEvent::ApprovalAutoApproved { id: "a-1".into() };
        let _e8 = AgentEvent::UnsupportedToolCall {
            id: "t-1".into(),
            name: "unknown".into(),
        };
        let _e9 = AgentEvent::Notification {
            message: "info".into(),
        };
        let _e10 = AgentEvent::OtherMessage {
            method: Some("other".into()),
            payload: serde_json::json!({}),
        };
        let _e11 = AgentEvent::Malformed { raw: "bad".into() };
    }

    #[test]
    fn turn_outcome_variants() {
        assert_eq!(TurnOutcome::Completed, TurnOutcome::Completed);
        assert_ne!(TurnOutcome::Completed, TurnOutcome::Cancelled);
    }
}
