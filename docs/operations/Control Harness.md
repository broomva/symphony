---
tags:
  - symphony
  - operations
  - control
  - testing
type: operations
status: active
area: quality
created: 2026-03-16
---

# Control Harness

Build gates, test coverage, and audit commands. See [[CONTROL]] for the full setpoint matrix (50 setpoints).

## Gates

| Command | What it checks | When to run |
|---------|---------------|-------------|
| `make smoke` | compile + clippy (warnings=errors) + all tests | Before every commit (enforced by hook) |
| `make check` | compile + clippy only | Quick feedback during development |
| `make test` | `cargo test --workspace` | After code changes |
| `make build` | `cargo build --release` | Before deployment |
| `make control-audit` | smoke + format check | Before PR |
| `make fmt` | auto-format code | Fix formatting issues |
| `make install` | install binary locally | Local development |

## Pre-Commit Hook

The gate is enforced automatically via `.githooks/pre-commit`. Activate with:

```bash
git config core.hooksPath .githooks
```

The hook runs `make smoke` + `cargo fmt --all -- --check` and blocks the commit if anything fails.

## Current Status (2026-03-16)

```
make smoke → SMOKE PASS
  cargo check --workspace → OK
  cargo clippy --workspace -- -D warnings → 0 warnings
  cargo test --workspace → 171 passed, 0 failed, 5 ignored
```

## Test Distribution

| Crate | Tests | Ignored | Coverage Focus |
|-------|-------|---------|---------------|
| [[docs/crates/symphony-core\|core]] | 4 | 0 | Sanitization, slot math |
| [[docs/crates/symphony-config\|config]] | 36 | 0 | Parsing, extraction, validation, templates |
| [[docs/crates/symphony-tracker\|tracker]] | 25 | 5 | Normalization, queries, GraphQL tool, real API |
| [[docs/crates/symphony-workspace\|workspace]] | 18 | 0 | Hooks, safety, lifecycle |
| [[docs/crates/symphony-agent\|agent]] | 16 | 0 | Protocol, handshake, events |
| [[docs/crates/symphony-orchestrator\|orchestrator]] | 22 | 0 | Eligibility, sort, concurrency, backoff |
| [[docs/crates/symphony-observability\|observability]] | 15 | 0 | Endpoints, health, auth, status codes |
| root (CLI) | 35 | 0 | CLI args, subcommands, output format |
| **Total** | **171** | **5** | |

## Opt-in Integration Tests

Require `LINEAR_API_KEY` environment variable:

```bash
LINEAR_API_KEY=lin_api_xxx cargo test --workspace -- --ignored
```

5 tests: real Linear API calls (fetch issues, GraphQL queries, auth errors).

## Controller Policy

From [[CONTROL]]:

- **Always**: run `make smoke` before committing (enforced by pre-commit hook)
- **Always**: add tests that verify spec behavior for new code
- **Before push**: update docs per [[CLAUDE|CLAUDE.md]] "Documentation Obligations"
- **If smoke fails**: fix errors before proceeding (no `#[allow]` without justification)
- **If new orchestrator state code**: verify single-authority mutation
- **If spec ambiguity**: implement conservative interpretation, document in [[PLANS]]
- **If phase completed**: update [[.planning/STATE|State]], [[.planning/REQUIREMENTS|Requirements]], run `make control-audit`

## Deviation Log

| Date | Setpoint | Issue | Resolution |
|------|----------|-------|------------|
| 2026-03-06 | S2 | Clippy: stub dead_code | `#[allow(dead_code)]` (justified: fields used when impl complete) |
| 2026-03-06 | S2 | Clippy: manual_strip etc. | Auto-fixed via `cargo clippy --fix` |
| 2026-03-06 | S10 | Rust 2024: `set_var` unsafe | Wrapped in `unsafe` block (single-threaded test) |
| 2026-03-16 | S2 | Unused import in workspace tests | Removed redundant `use std::path::PathBuf` |
| 2026-03-16 | S2 | Clippy: too_many_arguments | `#[allow(clippy::too_many_arguments)]` (justified: constructor groups related subsystems) |

## See Also

- [[CONTROL]] — full setpoint matrix (50 setpoints across 8 categories)
- [[.planning/REQUIREMENTS|Requirements]] — spec conformance checklist
- [[AGENTS]] — agent guidelines for development
- [[CLAUDE]] — conventions and documentation obligations
