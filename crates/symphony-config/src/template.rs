//! Liquid template engine for prompt rendering (Spec Sections 5.4, 12).
//!
//! Renders the prompt template with issue data and attempt number.
//! Uses strict mode: unknown variables and filters cause errors.

use liquid::model::Value as LiquidValue;
use liquid::Object;
use serde_json::Value as JsonValue;
use symphony_core::Issue;

/// Errors from template operations.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("template_parse_error: {0}")]
    ParseError(String),
    #[error("template_render_error: {0}")]
    RenderError(String),
}

/// The default fallback prompt when the body is empty (S5.4).
const FALLBACK_PROMPT: &str = "You are working on an issue from Linear.";

/// Parse and render a prompt template with issue data (Spec Section 5.4, 12).
///
/// Template variables:
/// - `issue` — all normalized fields including labels, blockers
/// - `attempt` — `nil` on first run, integer on retry
///
/// Strict mode: unknown variables/filters fail.
pub fn render_prompt(
    template_str: &str,
    issue: &Issue,
    attempt: Option<u32>,
) -> Result<String, TemplateError> {
    // Empty prompt body → fallback (S5.4)
    let template_source = if template_str.trim().is_empty() {
        FALLBACK_PROMPT
    } else {
        template_str
    };

    let template = liquid::ParserBuilder::with_stdlib()
        .build()
        .map_err(|e| TemplateError::ParseError(e.to_string()))?
        .parse(template_source)
        .map_err(|e| TemplateError::ParseError(e.to_string()))?;

    let mut globals = Object::new();

    // Build issue object for template (S12.2)
    let issue_obj = build_issue_object(issue);
    globals.insert("issue".into(), LiquidValue::Object(issue_obj));

    // Attempt: nil on first run, integer on retry (S12.3)
    match attempt {
        Some(n) => {
            globals.insert("attempt".into(), LiquidValue::scalar(n as i64));
        }
        None => {
            globals.insert("attempt".into(), LiquidValue::Nil);
        }
    }

    template
        .render(&globals)
        .map_err(|e| TemplateError::RenderError(e.to_string()))
}

/// Convert an Issue to a Liquid Object (S12.2).
/// All keys are strings. Nested arrays/maps are preserved for iteration.
fn build_issue_object(issue: &Issue) -> Object {
    let mut obj = Object::new();

    obj.insert("id".into(), LiquidValue::scalar(issue.id.clone()));
    obj.insert(
        "identifier".into(),
        LiquidValue::scalar(issue.identifier.clone()),
    );
    obj.insert("title".into(), LiquidValue::scalar(issue.title.clone()));
    obj.insert(
        "description".into(),
        issue
            .description
            .as_ref()
            .map(|d| LiquidValue::scalar(d.clone()))
            .unwrap_or(LiquidValue::Nil),
    );
    obj.insert(
        "priority".into(),
        issue
            .priority
            .map(|p| LiquidValue::scalar(p as i64))
            .unwrap_or(LiquidValue::Nil),
    );
    obj.insert("state".into(), LiquidValue::scalar(issue.state.clone()));
    obj.insert(
        "branch_name".into(),
        issue
            .branch_name
            .as_ref()
            .map(|b| LiquidValue::scalar(b.clone()))
            .unwrap_or(LiquidValue::Nil),
    );
    obj.insert(
        "url".into(),
        issue
            .url
            .as_ref()
            .map(|u| LiquidValue::scalar(u.clone()))
            .unwrap_or(LiquidValue::Nil),
    );

    // Labels: preserved as array for template iteration (S12.2)
    let labels: Vec<LiquidValue> = issue
        .labels
        .iter()
        .map(|l| LiquidValue::scalar(l.clone()))
        .collect();
    obj.insert("labels".into(), LiquidValue::Array(labels));

    // Blocked_by: preserved as array of objects (S12.2)
    let blockers: Vec<LiquidValue> = issue
        .blocked_by
        .iter()
        .map(|b| {
            let mut blocker_obj = Object::new();
            blocker_obj.insert(
                "id".into(),
                b.id.as_ref()
                    .map(|s| LiquidValue::scalar(s.clone()))
                    .unwrap_or(LiquidValue::Nil),
            );
            blocker_obj.insert(
                "identifier".into(),
                b.identifier
                    .as_ref()
                    .map(|s| LiquidValue::scalar(s.clone()))
                    .unwrap_or(LiquidValue::Nil),
            );
            blocker_obj.insert(
                "state".into(),
                b.state
                    .as_ref()
                    .map(|s| LiquidValue::scalar(s.clone()))
                    .unwrap_or(LiquidValue::Nil),
            );
            LiquidValue::Object(blocker_obj)
        })
        .collect();
    obj.insert("blocked_by".into(), LiquidValue::Array(blockers));

    // Timestamps as strings (S12.2)
    obj.insert(
        "created_at".into(),
        issue
            .created_at
            .map(|d| LiquidValue::scalar(d.to_rfc3339()))
            .unwrap_or(LiquidValue::Nil),
    );
    obj.insert(
        "updated_at".into(),
        issue
            .updated_at
            .map(|d| LiquidValue::scalar(d.to_rfc3339()))
            .unwrap_or(LiquidValue::Nil),
    );

    obj
}

/// Convert a serde_json::Value to a Liquid Value for template rendering.
fn _json_to_liquid(val: &JsonValue) -> LiquidValue {
    match val {
        JsonValue::Null => LiquidValue::Nil,
        JsonValue::Bool(b) => LiquidValue::scalar(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                LiquidValue::scalar(i)
            } else if let Some(f) = n.as_f64() {
                LiquidValue::scalar(f)
            } else {
                LiquidValue::scalar(n.to_string())
            }
        }
        JsonValue::String(s) => LiquidValue::scalar(s.clone()),
        JsonValue::Array(arr) => {
            LiquidValue::Array(arr.iter().map(_json_to_liquid).collect())
        }
        JsonValue::Object(map) => {
            let mut obj = Object::new();
            for (k, v) in map {
                let key = liquid::model::KString::from_string(k.clone());
                obj.insert(key, _json_to_liquid(v));
            }
            LiquidValue::Object(obj)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_core::{BlockerRef, Issue};

    fn make_test_issue() -> Issue {
        Issue {
            id: "id-1".into(),
            identifier: "ABC-123".into(),
            title: "Fix the bug".into(),
            description: Some("A detailed description".into()),
            priority: Some(1),
            state: "Todo".into(),
            branch_name: Some("fix/abc-123".into()),
            url: Some("https://linear.app/team/ABC-123".into()),
            labels: vec!["bug".into(), "urgent".into()],
            blocked_by: vec![BlockerRef {
                id: Some("id-2".into()),
                identifier: Some("ABC-100".into()),
                state: Some("Done".into()),
            }],
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn render_identifier() {
        let issue = make_test_issue();
        let result = render_prompt("Working on {{ issue.identifier }}", &issue, None).unwrap();
        assert_eq!(result, "Working on ABC-123");
    }

    #[test]
    fn render_multiple_fields() {
        let issue = make_test_issue();
        let template = "{{ issue.identifier }}: {{ issue.title }} ({{ issue.state }})";
        let result = render_prompt(template, &issue, None).unwrap();
        assert_eq!(result, "ABC-123: Fix the bug (Todo)");
    }

    #[test]
    fn render_labels_size() {
        let issue = make_test_issue();
        let result =
            render_prompt("Labels: {{ issue.labels | size }}", &issue, None).unwrap();
        assert_eq!(result, "Labels: 2");
    }

    #[test]
    fn render_labels_iteration() {
        let issue = make_test_issue();
        let template = "{% for label in issue.labels %}{{ label }} {% endfor %}";
        let result = render_prompt(template, &issue, None).unwrap();
        assert_eq!(result.trim(), "bug urgent");
    }

    #[test]
    fn attempt_nil_on_first_run() {
        let issue = make_test_issue();
        // When attempt is None, it should be nil in template
        let template = "{% if attempt %}retry {{ attempt }}{% else %}first run{% endif %}";
        let result = render_prompt(template, &issue, None).unwrap();
        assert_eq!(result, "first run");
    }

    #[test]
    fn attempt_integer_on_retry() {
        let issue = make_test_issue();
        let template = "{% if attempt %}retry {{ attempt }}{% else %}first run{% endif %}";
        let result = render_prompt(template, &issue, Some(3)).unwrap();
        assert_eq!(result, "retry 3");
    }

    #[test]
    fn empty_template_uses_fallback() {
        let issue = make_test_issue();
        let result = render_prompt("", &issue, None).unwrap();
        assert_eq!(result, "You are working on an issue from Linear.");
    }

    #[test]
    fn whitespace_only_template_uses_fallback() {
        let issue = make_test_issue();
        let result = render_prompt("   \n  \t  ", &issue, None).unwrap();
        assert_eq!(result, "You are working on an issue from Linear.");
    }

    #[test]
    fn invalid_template_syntax_returns_parse_error() {
        let issue = make_test_issue();
        let result = render_prompt("{% invalid_tag %}", &issue, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, TemplateError::ParseError(_)));
    }

    #[test]
    fn blocker_access() {
        let issue = make_test_issue();
        let template = "{% for b in issue.blocked_by %}{{ b.identifier }}{% endfor %}";
        let result = render_prompt(template, &issue, None).unwrap();
        assert_eq!(result, "ABC-100");
    }

    #[test]
    fn nil_optional_fields() {
        let mut issue = make_test_issue();
        issue.description = None;
        issue.priority = None;
        let template = "desc={{ issue.description }} pri={{ issue.priority }}";
        let result = render_prompt(template, &issue, None).unwrap();
        assert_eq!(result, "desc= pri=");
    }
}
