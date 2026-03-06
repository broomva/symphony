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
