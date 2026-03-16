---
tags:
  - symphony
  - planning
  - roadmap
aliases:
  - Roadmap Graph
  - Phase Dependency Graph
created: 2026-03-06
---

# ROADMAP.md - Symphony

See [[PLANS]] for detailed task definitions. See [[docs/roadmap/Project Status|Project Status]] for current state.

## Phase Dependency Graph

```
Phase 0 (Scaffold) ─── DONE
    │
    ├── Phase 1 (Config & Workflow) ── S5, S6, S12
    │       │
    │       ├── Phase 2 (Linear Tracker) ── S11
    │       │       │
    │       ├── Phase 3 (Workspace Manager) ── S9, S15
    │       │       │
    │       └───────┴── Phase 4 (Orchestrator Core) ── S7, S8, S14, S16
    │                       │
    │                       ├── Phase 5 (Agent Runner) ── S10
    │                       │       │
    │                       └── Phase 6 (Observability) ── S13
    │                               │
    └───────────────────────────────┴── Phase 7 (Integration Testing) ── S17, S18
```

## Phase Details

### Phase 0: Scaffold [DONE]
**Spec coverage**: S1-S4 (domain model, system overview)
**Deliverable**: Compilable workspace with core types, stubs, 25 tests
**Gate**: `make smoke` passes

### Phase 1: Config & Workflow
**Spec coverage**: S5 (Workflow Spec), S6 (Config), S12 (Prompt Construction)
**Deliverable**: Complete WORKFLOW.md loader, Liquid template engine, dispatch validation, file watcher
**Gate**: S17.1 test cases (13 items)
**Key algorithms**: None (config is declarative)
**Key risks**: Liquid crate compatibility with strict mode; YAML edge cases

### Phase 2: Linear Tracker Client
**Spec coverage**: S11 (Issue Tracker Integration)
**Deliverable**: GraphQL client with pagination, normalization, error mapping
**Gate**: S17.3 test cases (9 items)
**Key algorithms**: None (API client)
**Key risks**: Linear GraphQL schema drift; pagination integrity

### Phase 3: Workspace Manager
**Spec coverage**: S9 (Workspace Management), S15 (Security)
**Deliverable**: Full workspace lifecycle, hook execution, safety invariants
**Gate**: S17.2 test cases (9 items)
**Key algorithms**: None (filesystem operations)
**Key risks**: Path traversal attacks; hook timeout reliability

### Phase 4: Orchestrator Core
**Spec coverage**: S7 (State Machine), S8 (Scheduling), S14 (Failure Model), S16 (Reference Algorithms)
**Deliverable**: Poll loop, dispatch, retry queue, reconciliation, startup cleanup
**Gate**: S17.4 test cases (12 items)
**Key algorithms**:
- S16.1: `start_service()` — startup sequence
- S16.2: `on_tick()` — poll-and-dispatch tick
- S16.3: `reconcile_running_issues()` — stall + state refresh
- S16.4: `dispatch_issue()` — spawn worker + create running entry
- S16.5: `run_agent_attempt()` — workspace + prompt + multi-turn loop
- S16.6: `on_worker_exit()` + `on_retry_timer()` — retry scheduling
**Key risks**: Concurrent state mutations; timer handling; multi-turn lifecycle

### Phase 5: Agent Runner (Codex Integration)
**Spec coverage**: S10 (Agent Runner Protocol)
**Deliverable**: Subprocess management, JSON-RPC handshake, turn processing, tool handling
**Gate**: S17.5 test cases (12 items)
**Key algorithms**: Session handshake sequence; line-delimited parsing
**Key risks**: App-server protocol compatibility; approval handling; timeout edge cases

### Phase 6: Observability & HTTP Server
**Spec coverage**: S13 (Logging, Status, Observability)
**Deliverable**: Structured logging, HTTP API, dashboard, token accounting
**Gate**: S17.6 test cases (5 items)
**Key algorithms**: Token accounting (absolute totals, delta tracking)
**Key risks**: Runtime snapshot accuracy under concurrent access

### Phase 7: Integration Testing & CLI
**Spec coverage**: S17 (Test Matrix), S18 (Implementation Checklist)
**Deliverable**: End-to-end tests, CLI tests, optional real Linear integration
**Gate**: S17.7 (CLI, 5 items) + S17.8 (real integration, opt-in)
**Key risks**: Mock fidelity; real API availability
