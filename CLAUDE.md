---
tags:
  - symphony
  - meta
aliases:
  - Claude Rules
created: 2026-03-06
---

# CLAUDE.md - Symphony

## Project
Symphony is a Rust-based orchestration service for coding agents.
See `AGENTS.md` for architecture and `PLANS.md` for roadmap.

## Commands
- `make smoke` — compile + clippy + test (the gate)
- `make check` — cargo check + clippy
- `make test` — cargo test --workspace
- `make build` — cargo build --release

## Conventions
- Rust edition 2024, minimum rustc 1.85
- `thiserror` for library errors, `anyhow` for application errors
- `tracing` for structured logging (never `println!` or `log`)
- `tokio` for async runtime
- `serde` for all serialization
- Tests live in `#[cfg(test)] mod tests` at bottom of each file
- State normalization: `trim().to_lowercase()` for issue state comparisons
- Workspace keys: only `[A-Za-z0-9._-]`, replace others with `_`

## Spec Reference
The canonical spec is `/Users/broomva/Downloads/Symphony SPEC.md`.
Always consult it for behavioral requirements.

## Safety Rules
- Workspace paths MUST stay under workspace root (canonicalize + prefix check)
- Coding agent cwd MUST equal the per-issue workspace path
- Never log API tokens or secret values
- Hook scripts run with timeout enforcement
- `set_var`/`remove_var` require `unsafe` blocks in Rust 2024 edition

## Obsidian Vault & Knowledge Graph

This repository IS an Obsidian vault. The `.obsidian/` directory at root configures it.

### Vault Structure
- **Root `.md` files** — project governance: README, AGENTS, CLAUDE, PLANS, CONTROL, SPEC, WORKFLOW
- **`docs/`** — Obsidian-native documentation with wikilinks and knowledge graph
  - `docs/Symphony Index.md` — vault entry point and navigation hub
  - `docs/architecture/` — system design, crate map, domain model
  - `docs/operations/` — control harness, configuration reference
  - `docs/roadmap/` — project status, production roadmap
  - `docs/crates/` — per-crate documentation (one note per crate)
- **`.planning/`** — project state, requirements, roadmap graph

### Documentation Standards
- Use `[[wikilinks]]` to connect related notes (e.g., `[[SPEC]]`, `[[docs/crates/symphony-core|symphony-core]]`)
- Add YAML frontmatter with `tags`, `created` date on new documentation notes
- When adding a feature or making significant changes, update the relevant docs:
  - New crate or module → update `docs/crates/` and `docs/architecture/Crate Map.md`
  - New config option → update `docs/operations/Configuration Reference.md`
  - Phase completion → update `docs/roadmap/Project Status.md` and `.planning/STATE.md`
  - New setpoint or test → update `CONTROL.md` and `docs/operations/Control Harness.md`
- Keep the knowledge graph connected: every note should link to at least one other note
- Prefer updating existing notes over creating new ones
- Do not duplicate information already in the code or git history
