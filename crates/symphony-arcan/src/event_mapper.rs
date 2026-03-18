// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Maps Arcan EventRecord to Symphony AgentEvent.

use serde_json::Value;

/// A simplified event from the Arcan SSE stream.
#[derive(Debug, Clone)]
pub struct ArcanEvent {
    pub sequence: u64,
    pub kind: String,
    pub data: Value,
}

impl ArcanEvent {
    /// Parse an SSE data line into an ArcanEvent.
    pub fn from_sse_data(data: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(data).ok()?;
        let sequence = value.get("sequence")?.as_u64()?;
        let kind = Self::extract_kind(&value)?;
        Some(Self {
            sequence,
            kind,
            data: value,
        })
    }

    /// Extract the event kind from the JSON value.
    ///
    /// Handles both string kinds (`"kind": "RunCompleted"`) and tagged
    /// union kinds (`"kind": { "Text": { "content": "..." } }`).
    fn extract_kind(value: &Value) -> Option<String> {
        let kind_val = value.get("kind")?;
        // Try string first
        if let Some(s) = kind_val.as_str() {
            return Some(s.to_string());
        }
        // Try tagged union — take the first key
        kind_val
            .as_object()
            .and_then(|obj| obj.keys().next().cloned())
    }

    /// Check if this event represents a run completion.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "RunCompleted" | "RunFailed" | "RunCancelled"
        )
    }

    /// Extract text content if this is a text event.
    pub fn text_content(&self) -> Option<&str> {
        self.data
            .get("kind")
            .and_then(|k| k.get("Text"))
            .and_then(|t| t.get("content"))
            .and_then(|c| c.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_string_kind_event() {
        let data = r#"{"sequence": 1, "kind": "RunCompleted", "payload": {}}"#;
        let event = ArcanEvent::from_sse_data(data).unwrap();
        assert_eq!(event.sequence, 1);
        assert_eq!(event.kind, "RunCompleted");
        assert!(event.is_terminal());
    }

    #[test]
    fn parse_tagged_union_kind_event() {
        let data =
            r#"{"sequence": 5, "kind": {"Text": {"content": "Hello world"}}, "session_id": "s1"}"#;
        let event = ArcanEvent::from_sse_data(data).unwrap();
        assert_eq!(event.sequence, 5);
        assert_eq!(event.kind, "Text");
        assert!(!event.is_terminal());
        assert_eq!(event.text_content(), Some("Hello world"));
    }

    #[test]
    fn parse_run_failed_is_terminal() {
        let data = r#"{"sequence": 10, "kind": "RunFailed"}"#;
        let event = ArcanEvent::from_sse_data(data).unwrap();
        assert!(event.is_terminal());
    }

    #[test]
    fn parse_run_cancelled_is_terminal() {
        let data = r#"{"sequence": 3, "kind": "RunCancelled"}"#;
        let event = ArcanEvent::from_sse_data(data).unwrap();
        assert!(event.is_terminal());
    }

    #[test]
    fn non_terminal_event() {
        let data = r#"{"sequence": 2, "kind": "ToolInvoked"}"#;
        let event = ArcanEvent::from_sse_data(data).unwrap();
        assert!(!event.is_terminal());
    }

    #[test]
    fn invalid_json_returns_none() {
        assert!(ArcanEvent::from_sse_data("not json").is_none());
    }

    #[test]
    fn missing_sequence_returns_none() {
        let data = r#"{"kind": "RunCompleted"}"#;
        assert!(ArcanEvent::from_sse_data(data).is_none());
    }

    #[test]
    fn missing_kind_returns_none() {
        let data = r#"{"sequence": 1}"#;
        assert!(ArcanEvent::from_sse_data(data).is_none());
    }

    #[test]
    fn text_content_none_for_non_text_event() {
        let data = r#"{"sequence": 1, "kind": "RunCompleted"}"#;
        let event = ArcanEvent::from_sse_data(data).unwrap();
        assert!(event.text_content().is_none());
    }
}
