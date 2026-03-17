---
tags:
  - symphony
  - contributing
type: reference
status: active
area: community
aliases:
  - Contributing
created: 2026-03-06
---

# Contributing to Symphony

Thanks for your interest in contributing! Symphony is an open-source orchestration engine for coding agents, and we welcome contributions of all kinds.

## Quick Start

```bash
# Clone and build
git clone https://github.com/broomva/symphony.git
cd symphony
make smoke    # compile + lint + test (~2 min)
```

Requires Rust 1.88+ (edition 2024).

## Development Commands

```bash
make smoke    # Full gate: compile + clippy + test
make check    # Compile + clippy only
make test     # Run all tests
make build    # Release build
```

## What to Contribute

### Tracker Plugins

Symphony supports Linear and GitHub Issues. Adding Jira, GitLab, Asana, etc. is the highest-impact contribution.

To add a tracker:

1. Create a new file in `crates/symphony-tracker/src/` (e.g., `jira.rs`)
2. Implement the `TrackerClient` trait (4 methods: `fetch_candidate_issues`, `fetch_issues_by_states`, `fetch_issue_states_by_ids`, `set_issue_state`)
3. Register it in the `create_tracker()` factory in `crates/symphony-tracker/src/lib.rs`
4. Add the new `kind` to config validation in `crates/symphony-config/src/loader.rs`
5. Add tests in the same file under `#[cfg(test)] mod tests`
6. Add an example workflow in `examples/`
7. Add setpoints to `CONTROL.md` for the new tracker

### Agent Runner Compatibility

The agent runner works with any CLI that:
- Accepts input on stdin (JSON-RPC) or as CLI arguments
- Outputs line-delimited JSON on stdout
- Uses stderr for diagnostics (not parsed)

If you get Symphony working with a new agent, add an example workflow.

### Bug Fixes and Improvements

- Check the [spec reference](/Users/broomva/Downloads/Symphony%20SPEC.md) for behavioral requirements
- Add tests for any new logic (tests live in `#[cfg(test)] mod tests` at the bottom of each file)
- Run `make smoke` before submitting

## Control Metalayer

Symphony uses a **control metalayer** ([[CONTROL]]) as the grounding framework for all development. Before writing code, read the setpoints that your change affects. After writing code, verify those setpoints pass.

The loop:
1. **CHECK** `CONTROL.md` → which setpoints does your change affect?
2. **IMPLEMENT** → write code that satisfies those setpoints
3. **MEASURE** → run `make smoke`
4. **VERIFY** → all affected setpoints green?
5. **DOCUMENT** → add new setpoints for new behavior
6. **FEEDBACK** → log deviations if any setpoint was relaxed

If you add a new feature, add corresponding setpoints to `CONTROL.md`. This keeps the system verifiable across agent sessions and human contributors alike.

## Code Style

- `thiserror` for library errors, `anyhow` for application errors
- `tracing` for structured logging (never `println!` or `log`)
- `tokio` for async runtime
- `serde` for all serialization
- State normalization: `trim().to_lowercase()` for state comparisons
- Workspace keys: only `[A-Za-z0-9._-]`, replace others with `_`

## Pull Request Process

1. Fork and create a branch from `master`
2. Make your changes with tests
3. Run `make smoke` — it must pass
4. Open a PR with a clear description of what and why
5. Link to any relevant spec sections (e.g., "Implements S11.2")

## Architecture

See [[AGENTS]] for crate layout and design decisions.
See [[PLANS]] for the implementation roadmap.
See [[docs/operations/Control Harness|Control Harness]] for the build gate details.

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). Please be respectful and constructive.

## See Also

- [[EXTENDING]] — plugin architecture: adding trackers, agent runners, config sections
- [[docs/architecture/Crate Map|Crate Map]] — all crates with spec coverage
- [[CONTROL]] — quality setpoints your code must satisfy
- [[docs/operations/Configuration Reference|Configuration Reference]] — WORKFLOW.md format
- [[SPEC]] — behavioral requirements
