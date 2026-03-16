---
tags:
  - symphony
  - crate
  - orchestrator
  - scheduling
type: crate
status: active
area: orchestrator
created: 2026-03-16
---

# symphony-orchestrator

**Spec coverage**: S7 (State Machine), S8 (Scheduling), S14 (Failure Model), S16 (Algorithms)
**Path**: `crates/symphony-orchestrator/src/`
**Tests**: 22

The brain of Symphony. Implements the poll loop, dispatch, reconciliation, retry queue, and worker lifecycle.

## Source Files

| File | Lines | Purpose |
|------|-------|---------|
| `scheduler.rs` | 723 | Main event loop, tick, dispatch, worker lifecycle, retry timers |
| `dispatch.rs` | 349 | Eligibility rules, sorting, concurrency control |
| `reconcile.rs` | 170 | Stall detection, tracker state refresh, backoff formula |
| `lib.rs` | 9 | Module exports |

## Algorithms (S16)

| Algorithm | Function | Spec |
|-----------|----------|------|
| `start_service()` | Startup sequence: logging → observability → watch → validate → cleanup → tick | S16.1 |
| `on_tick()` | Reconcile → validate → fetch → sort → dispatch → schedule | S16.2 |
| `reconcile_running_issues()` | Stall detection + tracker state refresh | S16.3 |
| `dispatch_issue()` | Spawn worker → create running entry → claim → remove retry | S16.4 |
| `run_agent_attempt()` | Workspace → hooks → session → multi-turn loop | S16.5 |
| `on_worker_exit()` / `on_retry_timer()` | Exit handling + retry scheduling | S16.6 |

## Eligibility Rules (S8.2)

An issue is eligible for dispatch when:
- Has required fields: `id`, `identifier`, `title`, `state`
- State in `active_states` AND not in `terminal_states`
- Not in `running` map AND not in `claimed` set
- Global concurrency slots available
- Per-state concurrency slots available
- Not a Todo with non-terminal blockers

## Retry Backoff (S8.4)

| Scenario | Delay | Attempt |
|----------|-------|---------|
| Normal exit (continuation) | 1000ms fixed | Reset to 1 |
| Failure exit | `min(10000 * 2^(attempt-1), max_backoff)` | Incremented |
| Failure attempt 1 | 10s | |
| Failure attempt 2 | 20s | |
| Failure attempt 3 | 40s | |
| Failure attempt 10 | Capped at `max_retry_backoff_ms` (default 300s) | |

## Known Gap

- `scheduler.rs:174` — stall detection identifies stalled processes but does not yet terminate them (logs warning only). Tracked for [[docs/roadmap/Production Roadmap|production hardening]].

## See Also

- [[docs/architecture/Architecture Overview|Architecture Overview]] — dispatch cycle diagram
- [[CONTROL]] — setpoints S16-S22 (dispatch and scheduling)
- [[docs/crates/symphony-agent|symphony-agent]] — worker subprocess management
