---
tags:
  - symphony
  - crate
  - tracker
type: crate
status: active
area: tracker
created: 2026-03-16
---

# symphony-tracker

**Spec coverage**: S11 (Issue Tracker Integration)
**Path**: `crates/symphony-tracker/src/`
**Tests**: 68 (63 unit + 5 opt-in integration)

Multi-tracker adapter supporting Linear (GraphQL), GitHub Issues (REST), and local Markdown files with optional Lago journaling.

## Source Files

| File | Purpose |
|------|---------|
| `lib.rs` | `TrackerClient` trait, factory, error types |
| `linear.rs` | Linear GraphQL client, pagination, normalization |
| `github.rs` | GitHub REST client, label-based state mapping |
| `markdown.rs` | Local `.md` file tracker with Lago-compatible JSONL journal |
| `graphql_tool.rs` | `linear_graphql` tool extension for agent subprocess (S10.5) |

## Tracker Trait

Four required operations (S11.1):
1. `fetch_candidate_issues()` — active issues for the project
2. `fetch_issues_by_states()` — issues in specific states (for terminal cleanup)
3. `fetch_issue_states_by_ids()` — refresh states for running issues
4. `set_issue_state()` — transition an issue to a new state (for done_state)

## Built-in Trackers

### Linear (`kind: linear`)
- GraphQL API with cursor-based pagination
- Blocker detection via inverse `blocks` relations
- Requires `api_key` and `project_slug`

### GitHub Issues (`kind: github`)
- REST API with label-based state mapping
- PRs automatically filtered out
- Requires `api_key` (GITHUB_TOKEN) and `project_slug` (owner/repo)

### Markdown Files (`kind: markdown`)
- Reads `.md` files from a local directory
- YAML front matter contains issue metadata (id, title, state, priority, labels, blocked_by)
- State transitions rewrite the front matter in-place
- No API key required; `project_slug` is the directory path
- Optional Lago journaling via `endpoint` field

## Lago Journal Integration

When using the markdown tracker, every state transition and poll scan is logged to `{issues_dir}/.journal.jsonl` using Lago's `EventPayload::Custom` schema. This enables:

- **Audit trail**: Full history of state transitions with timestamps
- **Lago import**: JSONL entries are compatible with Lago's event format for future batch ingestion
- **Optional live forwarding**: When `endpoint` points to a running Lago daemon, a session is created on startup

Event types journaled:
- `symphony.tracker.state_transition` — issue_id, from_state, to_state, issue_title
- `symphony.tracker.scan` — issue_count, issues snapshot

## Normalization (S11.3)

- `labels` → all lowercase
- `blocked_by` → derived from inverse relations where type = "blocks" (Linear), or front matter (Markdown)
- `priority` → integer only (non-integer → None)
- `created_at`, `updated_at` → ISO-8601 parsed
- State comparison: `trim().to_lowercase()`

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
- [[EXTENDING]] — how to add new tracker kinds
