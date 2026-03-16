---
tags:
  - symphony
  - architecture
  - domain
created: 2026-03-16
---

# Domain Model

Core types from [[SPEC]] Section 4, implemented in [[docs/crates/symphony-core|symphony-core]].

## Issue (S4.1.1)

The normalized representation of a tracker issue. All fields populated by [[docs/crates/symphony-tracker|tracker]] normalization.

| Field | Type | Notes |
|-------|------|-------|
| `id` | String | Tracker's internal ID (GraphQL ID for Linear) |
| `identifier` | String | Human-readable key (e.g., `PROJ-123`) |
| `title` | String | Required |
| `description` | Option\<String\> | May be empty |
| `priority` | Option\<i32\> | Integer only; non-integer → None |
| `state` | String | Normalized: `trim().to_lowercase()` |
| `branch_name` | Option\<String\> | |
| `url` | Option\<String\> | |
| `labels` | Vec\<String\> | All lowercase |
| `blocked_by` | Vec\<String\> | IDs from inverse "blocks" relations |
| `created_at` | Option\<DateTime\> | ISO-8601 parsed |
| `updated_at` | Option\<DateTime\> | ISO-8601 parsed |

## OrchestratorState (S4.1.8)

Central scheduling state, mutated only by the orchestrator (single authority).

| Field | Type | Purpose |
|-------|------|---------|
| `running` | HashMap\<String, RunningEntry\> | issue_id → active worker |
| `claimed` | HashSet\<String\> | issue IDs claimed for retry |
| `retry_attempts` | HashMap\<String, RetryEntry\> | pending retry timers |
| `completed` | HashSet\<String\> | successfully completed issue IDs |
| `codex_totals` | TokenTotals | aggregate input/output/total tokens |
| `seconds_running` | f64 | cumulative agent runtime |

## Normalization Rules (S4.2)

| Rule | Function | Example |
|------|----------|---------|
| **Workspace key** | `[^A-Za-z0-9._-]` → `_` | `PROJ/feat#42` → `PROJ_feat_42` |
| **State comparison** | `trim().to_lowercase()` | `" Todo "` → `"todo"` |
| **Session ID** | `<thread_id>-<turn_id>` | `th_abc-tn_123` |

## Supporting Types

- **Workspace** (S4.1.4): `path`, `workspace_key`, `created_now`
- **RunAttempt** (S4.1.5): `issue_id`, `attempt`, `workspace_path`, `started_at`, `status`, `error`
- **LiveSession** (S4.1.6): `session_id`, `thread_id`, `turn_id`, `tokens`, `turn_count`
- **RetryEntry** (S4.1.7): `issue_id`, `identifier`, `attempt`, `due_at_ms`, `error`

## See Also

- [[docs/crates/symphony-core|symphony-core]] — implementation
- [[.planning/REQUIREMENTS|Requirements]] — conformance checklist
- [[SPEC]] S4 — canonical definition
