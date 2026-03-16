---
tags:
  - symphony
  - contributing
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

Requires Rust 1.85+ (edition 2024).

## Development Commands

```bash
make smoke    # Full gate: compile + clippy + test
make check    # Compile + clippy only
make test     # Run all tests
make build    # Release build
```

## What to Contribute

### Tracker Plugins

Symphony currently supports Linear. Adding GitHub Issues, Jira, GitLab, etc. is the highest-impact contribution.

To add a tracker:

1. Create a new file in `crates/symphony-tracker/src/` (e.g., `github.rs`)
2. Implement the `TrackerClient` trait (3 methods: `fetch_candidate_issues`, `fetch_issues_by_states`, `fetch_issue_states_by_ids`)
3. Add the new `kind` to config validation in `crates/symphony-config/src/loader.rs`
4. Add tests in the same file under `#[cfg(test)] mod tests`
5. Add an example workflow in `examples/`

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

## See Also

- [[docs/architecture/Crate Map|Crate Map]] — all crates with spec coverage
- [[CONTROL]] — quality setpoints your code must satisfy
- [[docs/operations/Configuration Reference|Configuration Reference]] — WORKFLOW.md format
- [[SPEC]] — behavioral requirements
