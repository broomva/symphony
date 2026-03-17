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
creates isolated per-issue workspaces, and runs coding agent sessions (Claude, Codex, etc.)
for each issue. It is a scheduler/runner, not a workflow engine.

## Architecture
Rust workspace with layered crates matching the spec's abstraction levels:

| Crate | Spec Layer | Responsibility |
|-------|-----------|----------------|
| `symphony-core` | Domain Model (S4) | Shared types: Issue, State, Session, Workspace |
| `symphony-config` | Config + Policy (S5-6) | WORKFLOW.md loader, typed config, file watcher |
| `symphony-tracker` | Integration (S11) | Linear GraphQL client, issue normalization |
| `symphony-workspace` | Execution (S9) | Per-issue directory lifecycle, hooks, safety invariants |
| `symphony-agent` | Execution (S10) | Agent subprocess, JSON-RPC + simple pipe modes |
| `symphony-orchestrator` | Coordination (S7-8) | Poll loop, dispatch, reconciliation, retry, drain |
| `symphony-observability` | Observability (S13) | Structured logging, HTTP server, dashboard, health, auth |
| `symphony-cli` (root) | CLI (S17.7) | Subcommands: start, stop, status, issues, validate, etc. |

## Key Design Decisions
- **In-memory state**: Orchestrator state is intentionally in-memory; recovery is tracker-driven
- **Single authority**: Only the orchestrator mutates scheduling state
- **Workspace isolation**: Coding agents run ONLY inside per-issue workspace directories
- **Dynamic reload**: WORKFLOW.md changes are detected and re-applied without restart
- **Liquid-compatible templates**: Strict variable/filter checking for prompt rendering
- **Graceful shutdown**: SIGTERM/SIGINT → drain mode → wait for workers → exit
- **Stall kill**: Worker abort handles tracked; stalled sessions killed + retried
- **Bearer auth**: Optional `SYMPHONY_API_TOKEN` protects `/api/v1/*`; health endpoints open

## Gathering Context from the Knowledge Graph

This repo is an Obsidian vault. All `.md` files form a wikilinked knowledge graph.

### How to orient before working:
1. **Start at `docs/Symphony Index.md`** — it links to everything
2. **Check `docs/roadmap/Project Status.md`** — current phase, test counts, known gaps
3. **Read the relevant `docs/crates/<name>.md`** — for the crate you're modifying
4. **Check `CONTROL.md`** — setpoints your changes must satisfy
5. **Check `PLANS.md`** — task breakdown for the current phase
6. **Traverse `[[wikilinks]]`** — follow links to find related context; the graph is designed for this

### Vault map:
```
Root governance:  CLAUDE.md  AGENTS.md  PLANS.md  CONTROL.md  EXTENDING.md  CONTRIBUTING.md
Docs index:       docs/Symphony Index.md
Architecture:     docs/architecture/Crate Map.md, Domain Model.md
Operations:       docs/operations/Control Harness.md, Configuration Reference.md
Roadmap:          docs/roadmap/Project Status.md, Production Roadmap.md
Per-crate:        docs/crates/symphony-core.md, symphony-config.md, ...
Planning state:   .planning/STATE.md, .planning/REQUIREMENTS.md
Examples:         examples/linear-claude.md, linear-codex.md, github-claude.md
```

## Development Commands
```bash
make smoke          # Compile + clippy + test (gate — runs in pre-commit hook)
make check          # Compile + clippy only
make test           # Run all workspace tests (includes CLI integration tests)
make build          # Release build
make control-audit  # Smoke + format check (before PR)
make fmt            # Auto-format code
make install        # Install binary locally

# CLI-specific testing
cargo test --test cli_integration          # Run CLI binary integration tests only
cargo test --test cli_integration -- init  # Run only init-related CLI tests
```

## Control Harness

The pre-commit hook at `.githooks/pre-commit` enforces the gate automatically. Activate:
```bash
git config core.hooksPath .githooks
```

### Before every commit (enforced by hook):
- `cargo check --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
- `cargo test --workspace` passes
- `cargo fmt --all -- --check` passes

### Before every push (agent obligation):
- Documentation updated per the rules in CLAUDE.md "Documentation Obligations"
- `CONTROL.md` deviation log updated if any setpoint was relaxed
- `docs/roadmap/Project Status.md` updated if changes are significant
- `docs/operations/Control Harness.md` test counts updated if tests were added

### The control loop:
```
Code change → make smoke (pre-commit) → tests pass → docs updated → push
     ↑                                                                 |
     └─── If smoke fails: fix before proceeding, never suppress ───────┘
```

### Harness validation:
- `make harness-audit` — validates governance files, hooks, CI, frontmatter, deviation log freshness
- `make entropy-check` — reports `#[allow]` count, TODO/FIXME/HACK markers, doc staleness, test count drift

## Consciousness Substrates

The development process is grounded in three substrates that provide persistent context:

| Substrate | Source | Content |
|-----------|--------|---------|
| Control Metalayer | `.control/policy.yaml`, `.control/state.json` | Behavioral governance: what MUST be true |
| Knowledge Graph | `docs/`, `CLAUDE.md`, `AGENTS.md`, `.planning/` | Declarative memory: what IS known |
| Episodic Memory | `docs/conversations/`, `~/.claude/projects/` | Episodic memory: what HAS happened |

On each session start, orient by reading the control state, then the knowledge graph, then recent conversation history. See [[docs/control/Consciousness Architecture|Consciousness Architecture]] for the full design and [[docs/control/Session Protocol|Session Protocol]] for the actionable protocol.

## Control Metalayer — Development Grounding

The control metalayer (`CONTROL.md`) is the **active grounding framework** for all agent work.

**Before every change:**
1. Read `CONTROL.md` → identify affected setpoints
2. Implement code that satisfies those setpoints
3. Run `make smoke` → verify sensors pass
4. Add new setpoints if adding new behavior
5. Update docs: `Project Status.md`, `STATE.md`, `Control Harness.md`
6. Log deviations if any setpoint was temporarily relaxed

**PR Review Loop:**
After pushing changes, agents must handle PR review comments:
1. Check PR for review comments (`gh api repos/.../pulls/.../comments`)
2. Fix code, accept suggestions, or reply with justification
3. Push fixes and repeat until PR is clean or max_turns exhausted
4. Link PR to the issue tracker (Linear/GitHub)

## Agent Guidelines
- The spec (Symphony SPEC.md) is the source of truth for all behavior
- Prefer editing existing crate code over creating new crates
- Each crate has its own test module; add tests for any new logic
- Structured logging: always include `issue_id`, `issue_identifier`, `session_id` in logs
- State normalization: always trim + lowercase when comparing issue states
- Path safety: always validate workspace paths stay under workspace root
- See `EXTENDING.md` for how to add new trackers or agent runners

## Self-Reference

`CLAUDE.md` and this file (`AGENTS.md`) define how agents interact with Symphony. They are loaded at the start of every Claude Code session. If you change conventions, control harness behavior, or documentation obligations — **update these files** so the next session inherits the knowledge. This is the meta-definition that keeps the control loop coherent across sessions.
