---
tags:
  - symphony
  - index
aliases:
  - Home
  - Symphony
created: 2026-03-16
---

# Symphony

A Rust-based orchestration service that polls an issue tracker, creates isolated per-issue workspaces, and runs coding agent sessions. Scheduler/runner, not a workflow engine.

## Navigation

### Project Core
- [[README]] — Project overview and quickstart
- [[SPEC]] — Canonical service specification (Draft v1)
- [[AGENTS]] — Architecture and agent guidelines
- [[CLAUDE]] — Development conventions and safety rules
- [[ARCHITECTURE]] — Open core model decision (Apache 2.0 engine + proprietary SaaS)
- [[CONTRIBUTING]] — How to contribute: build, test, extend

### Architecture
- [[docs/architecture/Architecture Overview|Architecture Overview]] — System flow, dispatch cycle, concurrency model
- [[docs/architecture/Crate Map|Crate Map]] — All 8 crates with status and responsibility
- [[docs/architecture/Domain Model|Domain Model]] — Core types: Issue, State, Session, Workspace

### Operations
- [[CONTROL]] — Control metalayer: setpoints, sensors, actuators
- [[docs/operations/Control Harness|Control Harness]] — Build gates, test coverage, audit commands
- [[docs/operations/Configuration Reference|Configuration Reference]] — WORKFLOW.md format and all settings
- [[WORKFLOW]] — Live workflow configuration for Stimulus project

### Planning
- [[PLANS]] — Implementation roadmap (Phases 0-9)
- [[docs/roadmap/Project Status|Project Status]] — Current state, test metrics, completion
- [[docs/roadmap/Production Roadmap|Production Roadmap]] — Path to managed service
- [[.planning/STATE|State]] — Phase completion history and decisions
- [[.planning/REQUIREMENTS|Requirements]] — Spec conformance checklist (100%)
- [[.planning/ROADMAP|Roadmap Graph]] — Phase dependency graph
- [[.planning/PROJECT|Project Vision]] — Vision, stack, constraints

### Crate Documentation
- [[docs/crates/symphony-core|symphony-core]] — Domain model (S4)
- [[docs/crates/symphony-config|symphony-config]] — Config and workflow (S5-6)
- [[docs/crates/symphony-tracker|symphony-tracker]] — Linear GraphQL client (S11)
- [[docs/crates/symphony-workspace|symphony-workspace]] — Workspace lifecycle (S9)
- [[docs/crates/symphony-agent|symphony-agent]] — Agent subprocess (S10)
- [[docs/crates/symphony-orchestrator|symphony-orchestrator]] — Dispatch and scheduling (S7-8)
- [[docs/crates/symphony-observability|symphony-observability]] — HTTP API and logging (S13)

## Quick Stats (2026-03-16)

| Metric | Value |
|--------|-------|
| Total lines of Rust | ~6,100 |
| Tests passing | 141 (136 + 5 opt-in) |
| Crates | 8 (7 library + 1 binary) |
| Spec conformance | 100% core + extensions |
| Phases complete | 0-7 (core done) |
| Current focus | Phase 8 (OSS) + Phase 9 (Cloud) |
| Gate command | `make smoke` |
