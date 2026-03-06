# AGENTS.md - Symphony

## Repository Purpose
Symphony is a long-running orchestration service that polls an issue tracker (Linear),
creates isolated per-issue workspaces, and runs coding agent sessions (Codex app-server)
for each issue. It is a scheduler/runner, not a workflow engine.

## Architecture
Rust workspace with layered crates matching the spec's abstraction levels:

| Crate | Spec Layer | Responsibility |
|-------|-----------|----------------|
| `symphony-core` | Domain Model (S4) | Shared types: Issue, State, Session, Workspace |
| `symphony-config` | Config + Policy (S5-6) | WORKFLOW.md loader, typed config, file watcher |
| `symphony-tracker` | Integration (S11) | Linear GraphQL client, issue normalization |
| `symphony-workspace` | Execution (S9) | Per-issue directory lifecycle, hooks, safety invariants |
| `symphony-agent` | Execution (S10) | Codex app-server subprocess, JSON-RPC protocol |
| `symphony-orchestrator` | Coordination (S7-8) | Poll loop, dispatch, reconciliation, retry queue |
| `symphony-observability` | Observability (S13) | Structured logging, optional HTTP server + API |
| `symphony` (root) | CLI (S17.7) | Binary entry point, CLI args, startup |

## Key Design Decisions
- **In-memory state**: Orchestrator state is intentionally in-memory; recovery is tracker-driven
- **Single authority**: Only the orchestrator mutates scheduling state
- **Workspace isolation**: Coding agents run ONLY inside per-issue workspace directories
- **Dynamic reload**: WORKFLOW.md changes are detected and re-applied without restart
- **Liquid-compatible templates**: Strict variable/filter checking for prompt rendering

## Development Commands
```bash
make smoke    # Compile + clippy + test (gate)
make check    # Full check without tests
make test     # Run all workspace tests
make build    # Release build
```

## Agent Guidelines
- The spec (Symphony SPEC.md) is the source of truth for all behavior
- Prefer editing existing crate code over creating new crates
- Each crate has its own test module; add tests for any new logic
- Structured logging: always include `issue_id`, `issue_identifier`, `session_id` in logs
- State normalization: always trim + lowercase when comparing issue states
- Path safety: always validate workspace paths stay under workspace root
