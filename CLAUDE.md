---
tags:
  - symphony
  - meta
type: reference
status: active
area: development
aliases:
  - Claude Rules
created: 2026-03-06
---

# CLAUDE.md - Symphony

## Project
Symphony is a Rust-based orchestration service for coding agents.
See `AGENTS.md` for architecture, `PLANS.md` for roadmap, `CONTROL.md` for quality gates.

## Gathering Context

Before starting work, orient yourself using the knowledge graph:

1. **Read this file** and `AGENTS.md` — conventions, architecture, obligations
2. **Check project state** — `docs/roadmap/Project Status.md` and `.planning/STATE.md`
3. **Explore relevant docs** — traverse `[[wikilinks]]` from the entry points:
   - `docs/Symphony Index.md` — vault navigation hub
   - `docs/architecture/Crate Map.md` — which crate owns what
   - `docs/crates/<crate-name>.md` — per-crate API, tests, spec coverage
   - `docs/operations/Configuration Reference.md` — WORKFLOW.md format
   - `docs/operations/Control Harness.md` — build gates, test counts
4. **Check CONTROL.md** — setpoints your changes must satisfy
5. **Check PLANS.md** — current phase and remaining tasks
6. **Check `.planning/REQUIREMENTS.md`** — spec conformance checklist

When in doubt about behavior, consult the canonical spec: `/Users/broomva/Downloads/Symphony SPEC.md`.

## Commands
- `make smoke` — compile + clippy + test (the gate, runs pre-commit)
- `make check` — cargo check + clippy
- `make test` — cargo test --workspace
- `make build` — cargo build --release
- `make control-audit` — smoke + format check (before PR)
- `make fmt` — auto-format code
- `make install` — install binary locally

## Control Loop (Pre-Commit Enforcement)

A pre-commit hook at `.githooks/pre-commit` runs `make smoke` + format check on every commit. Activate it with:
```bash
git config core.hooksPath .githooks
```

**Before every commit, the following must be true:**
1. `cargo check --workspace` — zero compile errors
2. `cargo clippy --workspace -- -D warnings` — zero warnings
3. `cargo test --workspace` — all tests pass
4. `cargo fmt --all -- --check` — code is formatted

**Before every push, additionally:**
5. Documentation is updated (see Documentation Obligations below)
6. `CONTROL.md` deviation log is current if any setpoint was relaxed
7. `docs/roadmap/Project Status.md` reflects changes if significant

## Conventions
- Rust edition 2024, minimum rustc 1.85
- `thiserror` for library errors, `anyhow` for application errors
- `tracing` for structured logging (never `println!` or `log`)
- `tokio` for async runtime
- `serde` for all serialization
- Tests live in `#[cfg(test)] mod tests` at bottom of each file
- State normalization: `trim().to_lowercase()` for issue state comparisons
- Workspace keys: only `[A-Za-z0-9._-]`, replace others with `_`

## Safety Rules
- Workspace paths MUST stay under workspace root (canonicalize + prefix check)
- Coding agent cwd MUST equal the per-issue workspace path
- Never log API tokens or secret values
- Hook scripts run with timeout enforcement
- `set_var`/`remove_var` require `unsafe` blocks in Rust 2024 edition
- `SYMPHONY_API_TOKEN` — optional bearer auth for HTTP API; never commit real tokens

## Obsidian Vault & Knowledge Graph

This repository IS an Obsidian vault. The `.obsidian/` directory at root configures it.

### Vault Structure
- **Root `.md` files** — project governance: README, AGENTS, CLAUDE, PLANS, CONTROL, EXTENDING, CONTRIBUTING
- **`docs/`** — Obsidian-native documentation with wikilinks and knowledge graph
  - `docs/Symphony Index.md` — vault entry point and navigation hub
  - `docs/architecture/` — system design, crate map, domain model
  - `docs/operations/` — control harness, configuration reference
  - `docs/roadmap/` — project status, production roadmap
  - `docs/crates/` — per-crate documentation (one note per crate)
- **`.planning/`** — project state, requirements, roadmap graph
- **`examples/`** — example WORKFLOW.md files for different tracker+agent combos

### Documentation Obligations (MUST do before push)
- Use `[[wikilinks]]` to connect related notes (e.g., `[[SPEC]]`, `[[docs/crates/symphony-core|symphony-core]]`)
- Add YAML frontmatter with `tags`, `created` date on new documentation notes
- When adding a feature or making significant changes, update the relevant docs:
  - New crate or module → update `docs/crates/` and `docs/architecture/Crate Map.md`
  - New config option → update `docs/operations/Configuration Reference.md`
  - New test or setpoint → update `CONTROL.md` and `docs/operations/Control Harness.md`
  - Phase/milestone completion → update `docs/roadmap/Project Status.md` and `.planning/STATE.md`
  - New endpoint → update `docs/operations/Configuration Reference.md`
- Keep the knowledge graph connected: every note should link to at least one other note
- Prefer updating existing notes over creating new ones
- Do not duplicate information already in the code or git history

### Self-Reference
This file (`CLAUDE.md`) and `AGENTS.md` are the meta-definition for how agents interact with this project. They are loaded automatically by Claude Code at the start of every conversation. If you change how the control harness works, how docs should be maintained, or how context should be gathered — **update these files** so future sessions inherit the knowledge.
