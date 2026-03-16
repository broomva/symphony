---
tags:
  - symphony
  - architecture
  - meta
type: reference
status: active
area: development
aliases:
  - Agents Guide
created: 2026-03-06
---

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

## Obsidian Vault & Documentation

This repository is an Obsidian vault (`.obsidian/` at root). All `.md` files form a knowledge graph navigable via wikilinks.

### Vault Navigation
- **Entry point**: `docs/Symphony Index.md` — links to all documentation
- **Architecture**: `docs/architecture/` — system design, crate map, domain model
- **Operations**: `docs/operations/` — control harness, configuration reference
- **Roadmap**: `docs/roadmap/` — project status, production roadmap
- **Crate docs**: `docs/crates/` — one note per crate with API, tests, spec coverage
- **Planning**: `.planning/` — project state, requirements checklist, phase graph

### Documentation Obligations
When working on this project, agents MUST:
1. **After adding a feature**: update the relevant `docs/crates/` note and `docs/roadmap/Project Status.md`
2. **After adding config**: update `docs/operations/Configuration Reference.md`
3. **After adding tests/setpoints**: update `CONTROL.md` and `docs/operations/Control Harness.md`
4. **After completing a phase**: update `.planning/STATE.md`, `.planning/REQUIREMENTS.md`, and `docs/roadmap/Project Status.md`
5. **Use wikilinks**: connect notes with `[[target]]` or `[[path/to/note|display text]]` syntax
6. **Add frontmatter**: new docs notes should have `tags`, `created` date in YAML frontmatter
7. **Keep graph connected**: every new note must link to at least one existing note
