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
- `make test` — cargo test --workspace (includes CLI integration tests)
- `make build` — cargo build --release
- `make control-audit` — smoke + format check (before PR)
- `make fmt` — auto-format code
- `make install` — install binary locally

## CLI Testing

Integration tests in `tests/cli_integration.rs` verify the binary end-to-end:
- `symphony init` scaffolds WORKFLOW.md correctly for both tracker types
- `symphony validate` catches missing keys, invalid config
- `symphony config` displays resolved settings (including pr_feedback, done_state)
- Init → Validate round-trip: generated workflows pass validation
- Remote daemon access: auth rejection works properly
- Error paths: missing files, unsupported trackers, overwrite protection

When adding new CLI subcommands or config options:
1. Add arg parsing test in `src/cli/mod.rs`
2. Add binary integration test in `tests/cli_integration.rs`
3. Verify with `cargo test --test cli_integration`

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

## Control Metalayer (Development Grounding Framework)

The control metalayer (`CONTROL.md`) is the **active grounding framework** that drives all development — not a passive checklist. Every code change follows this loop:

```
1. CHECK setpoints    → Which setpoints does this change affect?
2. IMPLEMENT          → Write code that satisfies the setpoints
3. MEASURE (sensors)  → Run `make smoke` / `make control-audit`
4. VERIFY             → All affected setpoints green?
5. DOCUMENT           → Update CONTROL.md, Project Status, STATE.md
6. FEEDBACK           → Deviation log if any setpoint was relaxed
```

Before starting any feature or fix:
- Read `CONTROL.md` to identify which setpoints your change touches
- After implementation, verify those setpoints pass
- If adding new behavior, add corresponding setpoints to CONTROL.md
- Update `docs/roadmap/Project Status.md` and `.planning/STATE.md` test counts

The metalayer ensures **stability across agent sessions** — every agent reads the same setpoints and produces verifiably correct output.

## PR Review Loop

When Symphony (or any agent) works on an issue, the full cycle includes PR review handling:

```
1. Agent implements the change
2. after_run hook: commit → push → create PR
3. pr_feedback hook: fetch PR review comments (gh pr view --json)
4. If comments exist → next turn receives them as context
5. Agent resolves comments → push fixes
6. Repeat until PR is clean or max_turns exhausted
```

**Convention for agents working in this repo:**
- After pushing a PR, check for review comments with `gh api repos/{owner}/{repo}/pulls/{number}/comments`
- Resolve each comment by either: fixing the code, accepting the suggestion, or replying with justification for rejecting
- PR title format: `<ISSUE-ID>: <concise description>`
- PR body must include: Summary, Files Changed, Tests, Test Plan
- Link the PR to the Linear issue via `gh` or Linear API

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
