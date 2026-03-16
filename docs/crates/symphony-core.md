---
tags:
  - symphony
  - crate
  - domain
created: 2026-03-16
---

# symphony-core

**Spec coverage**: S4 (Domain Model)
**Path**: `crates/symphony-core/src/`
**Tests**: 4

Shared domain types used by all other crates. See [[docs/architecture/Domain Model|Domain Model]] for type details.

## Source Files

| File | Purpose |
|------|---------|
| `lib.rs` | Module exports |
| `issue.rs` | `Issue` struct, `workspace_key()` sanitization |
| `state.rs` | `OrchestratorState`, `RunningEntry`, `RetryEntry`, slot calculations |
| `session.rs` | `LiveSession`, `TokenTotals`, `RunAttempt` |
| `workspace.rs` | `Workspace` struct (path, key, created_now) |

## Key Functions

- `Issue::workspace_key()` — sanitizes identifier per S4.2
- `OrchestratorState::available_slots()` — global concurrency check
- `OrchestratorState::available_slots_for_state()` — per-state concurrency check

## Dependencies

None (leaf crate — depended on by all others).
