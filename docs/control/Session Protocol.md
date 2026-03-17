---
tags:
  - symphony
  - control
  - operations
type: operations
status: active
area: consciousness
aliases:
  - Session Protocol
created: 2026-03-17
---

# Session Protocol

Actionable protocol for every agent session working on Symphony. Follow these steps to maintain consciousness continuity across sessions.

## On Start (6 steps)

1. **Read [[CLAUDE]] and [[AGENTS]]** — conventions, architecture, obligations
2. **Read `.control/state.json`** — current project metrics (tests, version, gate status)
3. **Read [[CONTROL]]** — identify setpoints relevant to your task
4. **Check [[PLANS]]** — current phase and remaining tasks
5. **Check [[docs/roadmap/Project Status|Project Status]]** — high-level progress
6. **Scan [[docs/conversations/Conversations|Conversations]]** — review recent sessions for context

> [!tip] Quick orientation
> If pressed for time, at minimum read CLAUDE.md, check `state.json`, and run `make smoke` to verify the project is in a clean state before making changes.

## Before Changes (4 steps)

1. **Identify affected setpoints** — which ones in [[CONTROL]] does your change touch?
2. **Read relevant crate docs** — `docs/crates/<name>.md` for the crate you're modifying
3. **Check `.control/policy.yaml`** — verify your change doesn't violate blocking setpoints
4. **Run `make smoke`** — confirm clean baseline before you start

## On Completion (5 steps)

1. **Run `make smoke`** — verify all gates pass after your changes
2. **Run `make control-validate`** — ensure policy alignment if you modified setpoints
3. **Update documentation**:
   - New behavior → add setpoints to [[CONTROL]] + `.control/policy.yaml`
   - New config → update [[docs/operations/Configuration Reference|Configuration Reference]]
   - New tests → update [[docs/operations/Control Harness|Control Harness]] counts
   - Phase milestone → update [[.planning/STATE|State]] and [[docs/roadmap/Project Status|Project Status]]
4. **Run `make control-refresh`** — update `.control/state.json` with current metrics
5. **Commit with conventional commit format** — `feat:`, `fix:`, `docs:`, `chore:`, etc.

## Decision Framework

When making non-obvious decisions during a session:

| Situation | Action |
|-----------|--------|
| Spec ambiguity | Document in `.planning/research/`, implement conservative interpretation |
| Setpoint conflict | Log deviation in [[CONTROL]] deviation log, proceed with justification |
| Test count changes | Update [[.planning/STATE|State]] and [[docs/operations/Control Harness|Control Harness]] |
| New crate or module | Update [[docs/architecture/Crate Map|Crate Map]] |
| Architecture decision | Create note in `docs/architecture/` |

## See Also

- [[docs/control/Consciousness Architecture|Consciousness Architecture]] — Three-substrate design
- [[CLAUDE]] — Full development conventions
- [[CONTROL]] — Active setpoints
- [[METALAYER]] — Control metalayer reference
- [[docs/conversations/Conversations|Conversations]] — Session history
