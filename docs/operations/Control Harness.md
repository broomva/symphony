---
tags:
  - symphony
  - operations
  - control
  - testing
created: 2026-03-16
---

# Control Harness

Build gates, test coverage, and audit commands. See [[CONTROL]] for the full setpoint matrix.

## Gates

| Command | What it checks | When to run |
|---------|---------------|-------------|
| `make smoke` | compile + clippy (warnings=errors) + all tests | Before every commit |
| `make check` | compile + clippy only | Quick feedback during development |
| `make test` | `cargo test --workspace` | After code changes |
| `make build` | `cargo build --release` | Before deployment |
| `make control-audit` | smoke + format check | Before PR |

## Current Status (2026-03-16)

```
make smoke → SMOKE PASS
  cargo check --workspace → OK
  cargo clippy --workspace -- -D warnings → 0 warnings
  cargo test --workspace → 136 passed, 0 failed, 5 ignored
```

## Test Distribution

| Crate | Tests | Coverage Focus |
|-------|-------|---------------|
| [[docs/crates/symphony-core\|core]] | 4 | Sanitization, slot math |
| [[docs/crates/symphony-config\|config]] | 36 | Parsing, extraction, validation, templates |
| [[docs/crates/symphony-tracker\|tracker]] | 30 | Normalization, queries, GraphQL tool |
| [[docs/crates/symphony-workspace\|workspace]] | 18 | Hooks, safety, lifecycle |
| [[docs/crates/symphony-agent\|agent]] | 16 | Protocol, handshake, events |
| [[docs/crates/symphony-orchestrator\|orchestrator]] | 22 | Eligibility, sort, concurrency, backoff |
| [[docs/crates/symphony-observability\|observability]] | 5 | Endpoints, status codes |
| root (`main.rs`) | 5 | CLI args, port config |

## Opt-in Integration Tests

Require `LINEAR_API_KEY` environment variable:

```bash
LINEAR_API_KEY=lin_api_xxx cargo test --workspace -- --ignored
```

5 tests: real Linear API calls (fetch issues, GraphQL queries, auth errors).

## Controller Policy

From [[CONTROL]]:

- **Always**: run `make smoke` before committing
- **Always**: add tests that verify spec behavior for new code
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

## See Also

- [[CONTROL]] — full setpoint matrix (42 setpoints)
- [[.planning/REQUIREMENTS|Requirements]] — spec conformance checklist
- [[AGENTS]] — agent guidelines for development
