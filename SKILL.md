---
name: symphony
description: >
  Rust orchestration engine for coding agents. Polls issue trackers
  (Linear, GitHub), creates isolated per-issue workspaces, and runs
  coding agent sessions with lifecycle hooks, retry/backoff, concurrency
  control, and a live HTTP dashboard. Includes control metalayer
  (CONTROL.md / setpoints) for grounded development workflows.
trigger_words:
  - symphony
  - coding agent orchestration
  - issue tracker automation
  - WORKFLOW.md
  - control metalayer
  - agent dispatch
  - lifecycle hooks
  - Linear automation
  - agent orchestrator
---

# symphony

Rust orchestration engine that polls issue trackers (Linear, GitHub), creates
isolated workspaces, and runs coding agents automatically.

## Install

```bash
cargo install symphony-cli                # from source
curl -fsSL https://raw.githubusercontent.com/broomva/symphony/master/install.sh | sh  # binary
docker pull ghcr.io/broomva/symphony:latest  # container
```

## Quick Start

```bash
symphony init                          # scaffold WORKFLOW.md (Linear default)
symphony init --tracker github          # GitHub Issues template
symphony validate WORKFLOW.md           # verify config
symphony start WORKFLOW.md              # run daemon
```

## Key Capabilities

- **Tracker integration** -- Linear and GitHub Issues out of the box; extensible
  via `TrackerClient` trait.
- **Lifecycle hooks** -- `after_create`, `before_run`, `after_run`,
  `before_remove` with timeout enforcement.
- **Control metalayer** -- CONTROL.md with setpoints, sensors, and feedback
  loops for grounded agent development.
- **Concurrency & retry** -- Slot-based dispatch with configurable
  `max_concurrent_agents`; exponential backoff on failure.
- **Live dashboard** -- HTTP server with HTML dashboard and REST API for
  state, issues, and manual refresh.
- **Arcan runtime** -- Optional dispatch through the Arcan HTTP daemon instead
  of local subprocesses.

## Agent Lifecycle

```
Poll tracker -> fetch active issues -> sort by priority -> dispatch workers
    |-- after_create hook (clone repo)
    |-- before_run hook (rebase)
    |-- render prompt + run agent (max_turns)
    |-- after_run hook (commit, push, create PR)
    |-- pr_feedback hook (capture review comments)
    |-- done_state transition (auto-close issue)
    +-- retry (1s continuation / exponential backoff)
```

## License

Apache-2.0
