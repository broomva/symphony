---
tags:
  - symphony
  - architecture
type: architecture
status: active
area: system
aliases:
  - Architecture
created: 2026-03-16
---

# Architecture Overview

Symphony follows a **layered crate architecture** matching the [[SPEC]]'s abstraction levels. See [[docs/architecture/Crate Map|Crate Map]] for per-crate details.

## System Flow

```
                        ┌──────────────────┐
                        │   CLI (main.rs)  │
                        │  args → startup  │
                        └────────┬─────────┘
                                 │
                        ┌────────▼─────────┐
                        │   Orchestrator   │
                        │   (S7-8, S16)    │
                        │  poll → dispatch │
                        │  → reconcile →   │
                        │  retry           │
                        └─┬──┬──┬──┬───────┘
                          │  │  │  │
              ┌───────────┘  │  │  └───────────┐
              ▼              ▼  ▼              ▼
        ┌──────────┐  ┌────────┐ ┌────────┐ ┌──────────┐
        │  Config  │  │Tracker │ │Workspce│ │  Agent   │
        │  (S5-6)  │  │ (S11)  │ │ (S9)   │ │  (S10)   │
        │ WORKFLOW │  │ Linear │ │ dirs + │ │subprocess│
        │  loader  │  │GraphQL │ │ hooks  │ │ JSON-RPC │
        └────┬─────┘  └───┬────┘ └───┬────┘ └────┬─────┘
             │             │          │            │
             └─────────────┴────┬─────┴────────────┘
                                │
                       ┌────────▼─────────┐
                       │   Core Domain    │
                       │      (S4)        │
                       │ Issue, State,    │
                       │ Session, WS      │
                       └────────┬─────────┘
                                │
                       ┌────────▼─────────┐
                       │  Observability   │
                       │     (S13)        │
                       │ tracing, HTTP,   │
                       │ dashboard, API   │
                       └──────────────────┘
```

## Dispatch Cycle (per tick)

Each tick follows [[SPEC]] Algorithm 16.2:

1. **Reconcile** — stall detection + tracker state refresh ([[docs/crates/symphony-orchestrator|orchestrator]]`.reconcile`)
2. **Validate** — config preflight; skip dispatch if invalid, keep reconciliation
3. **Fetch** — candidate issues from Linear ([[docs/crates/symphony-tracker|tracker]]`.linear`)
4. **Sort** — priority ASC (null last) → created_at oldest → identifier lexicographic
5. **Dispatch** — while global + per-state concurrency slots available
6. **Schedule** — next tick at `polling.interval_ms`

## Worker Lifecycle

Per [[SPEC]] Algorithm 16.5:

1. Create/reuse workspace directory → [[docs/crates/symphony-workspace|workspace]]
2. Run `after_create` hook (new workspace only; failure = fatal)
3. Run `before_run` hook (failure = abort attempt)
4. Start agent subprocess → [[docs/crates/symphony-agent|agent]] JSON-RPC handshake
5. Multi-turn loop: prompt → agent works → check tracker state → continue/stop
6. Run `after_run` hook (failure = ignored)
7. On exit: schedule retry (continuation 1s, failure exponential backoff)

## Concurrency Model

Defined in [[SPEC]] S8.3, implemented in [[docs/crates/symphony-orchestrator|orchestrator]]`.dispatch`:

- **Global limit**: `max_concurrent_agents` caps total running workers
- **Per-state limit**: `max_concurrent_agents_by_state[normalized_state]`
- **Claimed set**: prevents duplicate dispatches during retry delays
- **Single authority**: all state mutations on orchestrator's async task (no concurrent writes)

## Key Design Decisions

| Decision | Rationale | See |
|----------|-----------|-----|
| In-memory state | Recovery is tracker-driven; restart re-polls | [[.planning/STATE\|State]] |
| Single authority | No concurrent state mutations | [[SPEC]] S7 |
| Workspace isolation | Agents run only in per-issue dirs | [[docs/crates/symphony-workspace\|workspace]] |
| Dynamic reload | WORKFLOW.md changes without restart | [[docs/operations/Configuration Reference\|Config]] |
| Liquid templates | Strict variable/filter checking | [[docs/crates/symphony-config\|config]] |

## See Also

- [[docs/architecture/Crate Map|Crate Map]] — detailed per-crate breakdown
- [[docs/architecture/Domain Model|Domain Model]] — core types
- [[docs/operations/Control Harness|Control Harness]] — build gates and CI
