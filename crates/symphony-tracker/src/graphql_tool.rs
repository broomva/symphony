// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Optional `linear_graphql` client-side tool extension (Spec Section 10.5).
//!
//! Allows the coding agent to execute GraphQL queries/mutations against Linear
//! using Symphony's configured tracker auth.

use serde_json::Value;

/// Result of a `linear_graphql` tool call.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphqlToolResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Validate the input for a `linear_graphql` tool call (S10.5).
///
/// Returns `Ok((query, variables))` or `Err(error_message)`.
pub fn validate_input(input: &Value) -> Result<(String, Value), String> {
    // Accept either an object with "query" field or a raw string
    let (query_str, variables) = if let Some(s) = input.as_str() {
        // Raw query string shorthand
        (s.to_string(), Value::Null)
    } else if let Some(obj) = input.as_object() {
        let query = obj
            .get("query")
            .and_then(|q| q.as_str())
            .ok_or("'query' must be a non-empty string")?;

        if query.trim().is_empty() {
            return Err("'query' must be a non-empty string".into());
        }

        let vars = obj.get("variables").cloned().unwrap_or(Value::Null);
        // Variables must be an object when present (S10.5)
        if !vars.is_null() && !vars.is_object() {
            return Err("'variables' must be a JSON object when present".into());
        }

        (query.to_string(), vars)
    } else {
        return Err("input must be an object with 'query' field or a raw query string".into());
    };

    if query_str.trim().is_empty() {
        return Err("'query' must be a non-empty string".into());
    }

    // Check for multiple operations (S10.5: single operation only)
    if has_multiple_operations(&query_str) {
        return Err("query must contain exactly one GraphQL operation".into());
    }

    Ok((query_str, variables))
}

/// Check if a GraphQL document contains multiple operations.
///
/// Simple heuristic: count top-level `query`, `mutation`, `subscription` keywords.
fn has_multiple_operations(query: &str) -> bool {
    let mut count = 0;
    let mut chars = query.chars().peekable();
    let mut in_string = false;
    let mut in_comment = false;

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '\n' {
                in_comment = false;
            }
            continue;
        }
        if ch == '#' {
            in_comment = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }

        // Look for operation keywords at word boundaries
        if ch.is_alphabetic() {
            let mut word = String::new();
            word.push(ch);
            while let Some(&next) = chars.peek() {
                if next.is_alphanumeric() || next == '_' {
                    word.push(next);
                    chars.next();
                } else {
                    break;
                }
            }
            match word.as_str() {
                "query" | "mutation" | "subscription" => {
                    count += 1;
                    if count > 1 {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }

    false
}

/// Execute a `linear_graphql` tool call against the Linear API.
///
/// Uses the provided endpoint and API key (from Symphony's tracker config).
pub async fn execute_graphql_tool(
    endpoint: &str,
    api_key: &str,
    query: &str,
    variables: Value,
) -> GraphqlToolResult {
    let http = reqwest::Client::new();
    let body = serde_json::json!({
        "query": query,
        "variables": if variables.is_null() { Value::Object(serde_json::Map::new()) } else { variables },
    });

    let response = match http
        .post(endpoint)
        .header("Authorization", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return GraphqlToolResult {
                success: false,
                data: None,
                errors: None,
                error: Some(format!("transport_failure: {e}")),
            };
        }
    };

    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".into());
        return GraphqlToolResult {
            success: false,
            data: None,
            errors: None,
            error: Some(format!("api_status_{status}: {body_text}")),
        };
    }

    let json: Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            return GraphqlToolResult {
                success: false,
                data: None,
                errors: None,
                error: Some(format!("parse_failure: {e}")),
            };
        }
    };

    // Check for top-level GraphQL errors (S10.5)
    let has_errors = json
        .get("errors")
        .and_then(|e| e.as_array())
        .is_some_and(|arr| !arr.is_empty());

    if has_errors {
        // success=false but preserve full body for debugging
        GraphqlToolResult {
            success: false,
            data: json.get("data").cloned(),
            errors: json.get("errors").cloned(),
            error: None,
        }
    } else {
        GraphqlToolResult {
            success: true,
            data: json.get("data").cloned(),
            errors: None,
            error: None,
        }
    }
}

/// Tool spec for advertising `linear_graphql` during handshake (S10.5).
pub fn tool_spec() -> Value {
    serde_json::json!({
        "name": "linear_graphql",
        "description": "Execute a GraphQL query or mutation against the Linear API.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "A single GraphQL query or mutation document"
                },
                "variables": {
                    "type": "object",
                    "description": "Optional GraphQL variables"
                }
            },
            "required": ["query"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_valid_query() {
        let input = serde_json::json!({
            "query": "query { viewer { id } }",
        });
        let (query, vars) = validate_input(&input).unwrap();
        assert_eq!(query, "query { viewer { id } }");
        assert!(vars.is_null());
    }

    #[test]
    fn validate_query_with_variables() {
        let input = serde_json::json!({
            "query": "query($id: ID!) { issue(id: $id) { title } }",
            "variables": { "id": "abc-123" }
        });
        let (query, vars) = validate_input(&input).unwrap();
        assert!(query.contains("$id: ID!"));
        assert!(vars.is_object());
        assert_eq!(vars["id"], "abc-123");
    }

    #[test]
    fn validate_raw_string_input() {
        let input = Value::String("query { viewer { id } }".into());
        let (query, _vars) = validate_input(&input).unwrap();
        assert_eq!(query, "query { viewer { id } }");
    }

    #[test]
    fn validate_empty_query_fails() {
        let input = serde_json::json!({ "query": "" });
        let err = validate_input(&input).unwrap_err();
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn validate_missing_query_fails() {
        let input = serde_json::json!({ "variables": {} });
        let err = validate_input(&input).unwrap_err();
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn validate_variables_must_be_object() {
        let input = serde_json::json!({
            "query": "query { viewer { id } }",
            "variables": [1, 2, 3]
        });
        let err = validate_input(&input).unwrap_err();
        assert!(err.contains("JSON object"));
    }

    #[test]
    fn validate_multiple_operations_rejected() {
        let input = serde_json::json!({
            "query": "query A { viewer { id } } mutation B { updateIssue { id } }"
        });
        let err = validate_input(&input).unwrap_err();
        assert!(err.contains("exactly one"));
    }

    #[test]
    fn validate_single_mutation_accepted() {
        let input = serde_json::json!({
            "query": "mutation { updateIssue(id: \"123\", input: { title: \"New\" }) { success } }"
        });
        assert!(validate_input(&input).is_ok());
    }

    #[test]
    fn has_multiple_operations_detects_two() {
        assert!(has_multiple_operations("query A { a } mutation B { b }"));
    }

    #[test]
    fn has_multiple_operations_single_ok() {
        assert!(!has_multiple_operations("query { viewer { id } }"));
    }

    #[test]
    fn has_multiple_operations_ignores_comments() {
        // "mutation" in a comment should not count
        assert!(!has_multiple_operations(
            "query { viewer { id } }\n# mutation { x }"
        ));
    }

    #[test]
    fn tool_spec_has_required_fields() {
        let spec = tool_spec();
        assert_eq!(spec["name"], "linear_graphql");
        assert!(spec.get("inputSchema").is_some());
    }

    #[test]
    fn graphql_tool_result_serialization() {
        let result = GraphqlToolResult {
            success: true,
            data: Some(serde_json::json!({"viewer": {"id": "user-1"}})),
            errors: None,
            error: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["success"], true);
        assert!(json.get("data").is_some());
        // None fields should be skipped
        assert!(json.get("errors").is_none());
        assert!(json.get("error").is_none());
    }

    #[test]
    fn graphql_tool_result_failure() {
        let result = GraphqlToolResult {
            success: false,
            data: Some(serde_json::json!(null)),
            errors: Some(serde_json::json!([{"message": "Not found"}])),
            error: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["success"], false);
        assert!(json.get("errors").is_some());
    }

    // ─── Real Integration Tests (S17.8) ───

    #[tokio::test]
    #[ignore] // S17.8: skipped when credentials absent
    async fn real_graphql_tool_valid_query() {
        let api_key = std::env::var("LINEAR_API_KEY")
            .expect("LINEAR_API_KEY must be set for real integration tests");

        let result = execute_graphql_tool(
            "https://api.linear.app/graphql",
            &api_key,
            "query { viewer { id name } }",
            Value::Null,
        )
        .await;

        assert!(result.success, "valid query should succeed: {:?}", result);
        assert!(result.data.is_some(), "data should be present");
        assert!(result.error.is_none(), "error should be absent on success");
    }

    #[tokio::test]
    #[ignore] // S17.8: skipped when credentials absent
    async fn real_graphql_tool_invalid_auth() {
        let result = execute_graphql_tool(
            "https://api.linear.app/graphql",
            "lin_api_invalid_key_12345",
            "query { viewer { id } }",
            Value::Null,
        )
        .await;

        assert!(!result.success, "invalid auth should fail");
    }
}
