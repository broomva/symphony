---
tags:
  - symphony
  - crate
  - linear
  - graphql
created: 2026-03-16
---

# symphony-tracker

**Spec coverage**: S11 (Issue Tracker Integration)
**Path**: `crates/symphony-tracker/src/`
**Tests**: 30 (25 unit + 5 opt-in integration)

Linear GraphQL client with pagination, issue normalization, and error mapping.

## Source Files

| File | Lines | Purpose |
|------|-------|---------|
| `linear.rs` | 760 | HTTP client, GraphQL queries, pagination, normalization |
| `graphql_tool.rs` | 383 | `linear_graphql` tool extension for agent subprocess (S10.5) |
| `lib.rs` | 47 | `Tracker` trait, module exports |

## Tracker Trait

Three required operations (S11.1):
1. `fetch_candidate_issues()` — active issues for the project
2. `fetch_issues_by_states()` — issues in specific states (for terminal cleanup)
3. `fetch_issue_states_by_ids()` — refresh states for running issues

## Normalization (S11.3)

- `labels` → all lowercase
- `blocked_by` → derived from inverse relations where type = "blocks"
- `priority` → integer only (non-integer → None)
- `created_at`, `updated_at` → ISO-8601 parsed

## GraphQL Tool Extension (S10.5)

Available when `tracker.kind == "linear"`. Allows the coding agent to query Linear directly:
- Input validation: non-empty query, single operation, optional variables (must be object)
- Multi-operation queries rejected
- Reuses configured Linear endpoint + auth

## Integration Tests

Require `LINEAR_API_KEY` env var, run with `cargo test -- --ignored`:
- `real_linear_fetch_issues` — paginated candidate fetch
- `real_linear_graphql_query` — raw GraphQL execution
- `real_linear_invalid_key_returns_error` — auth error handling

## See Also

- [[docs/architecture/Domain Model|Domain Model]] — Issue normalization rules
- [[docs/operations/Configuration Reference|Configuration Reference]] — tracker config section
