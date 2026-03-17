---
tags:
  - symphony
  - control
  - quality
type: operations
status: active
area: quality
aliases:
  - Control
  - Control Metalayer
created: 2026-03-06
---

# CONTROL.md - Symphony Control Metalayer

> [!important] This file is the active grounding framework for all development
> It is NOT a passive checklist. Every agent session, every code change, and every PR must be grounded in these setpoints. Read this before writing code. Update it after adding behavior.

## How to Use This File

This control metalayer drives the development feedback loop:

```
1. CHECK    → Read the setpoints below. Which ones does your change affect?
2. IMPLEMENT → Write code that satisfies those setpoints
3. MEASURE  → Run `make smoke` (sensors below verify setpoints)
4. VERIFY   → All affected setpoints green? If not, fix before committing
5. DOCUMENT → Update this file if adding new behavior (new setpoints)
6. FEEDBACK → Log deviations if any setpoint was temporarily relaxed
```

> [!info] Operational companion
> For build commands, test distribution, and deviation history see [[docs/operations/Control Harness|Control Harness]]. For the implementation roadmap these setpoints verify, see [[PLANS]].

## Setpoints (What MUST be true)

### Build Quality
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S1 | Zero compile errors | `cargo check --workspace` exit 0 | — |
| S2 | Zero clippy warnings | `cargo clippy --workspace -- -D warnings` exit 0 | — |
| S3 | All tests pass | `cargo test --workspace` exit 0 | — |

### Domain Model (S4)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S4 | Issue entity has all 12 fields | Unit test: Issue struct construction | S4.1.1 |
| S5 | Workspace key sanitization: only `[A-Za-z0-9._-]` | Unit test: special chars → `_` | S4.2, S9.5 |
| S6 | State normalization: trim+lowercase before comparison | Unit test: " Todo " == "todo" | S4.2 |
| S7 | Session ID = `<thread_id>-<turn_id>` | Unit test: composition | S4.2 |

### Config & Workflow (S5, S6)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S8 | Front matter parsed as map; non-map = error | Unit test: list YAML → FrontMatterNotMap | S5.2 |
| S9 | Missing WORKFLOW.md = MissingFile error | Unit test: nonexistent path → error | S5.1 |
| S10 | `$VAR` resolution expands env vars | Unit test: `$TEST_KEY` → value | S6.1 |
| S11 | Empty `$VAR` = treated as missing | Unit test: unset var → empty string | S6.1 |
| S12 | `~` expanded to HOME in paths | Unit test: `~/foo` → `/home/user/foo` | S6.1 |
| S13 | Unknown template variables fail rendering | Unit test: `{{ unknown }}` → error | S5.4 |
| S14 | Dispatch validation catches missing tracker.kind | Unit test: empty kind → error | S6.3 |
| S15 | Invalid reload keeps last good config | Integration test: bad edit → no crash | S6.2 |

### Dispatch & Scheduling (S8)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S16 | Todo + non-terminal blocker = ineligible | Unit test: blocked todo → false | S8.2 |
| S17 | Todo + all terminal blockers = eligible | Unit test: unblocked todo → true | S8.2 |
| S18 | Sort: priority ASC, null last | Unit test: [3,1,null] → [1,3,null] | S8.2 |
| S19 | Continuation retry = 1000ms fixed | Unit test: backoff(n, max, true) = 1000 | S8.4 |
| S20 | Failure retry = min(10000*2^(n-1), max) | Unit test: attempt=3 → 40000 | S8.4 |
| S21 | Backoff capped at max_retry_backoff_ms | Unit test: attempt=10 → 300000 | S8.4 |
| S22 | Reconciliation runs BEFORE dispatch each tick | Integration test: tick ordering | S8.1 |

### Workspace Safety (S9, S15)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S23 | Workspace path must be under workspace root | Unit test: outside root → error | S9.5 |
| S24 | Agent cwd must equal workspace path | Validation before launch | S9.5 |
| S25 | `after_create` failure removes partial workspace | Unit test: hook fail → dir cleaned | S9.4 |
| S26 | `before_run` failure aborts run attempt | Unit test: hook fail → error returned | S9.4 |
| S27 | `after_run` failure is logged and ignored | Unit test: hook fail → no error propagated | S9.4 |
| S28 | Hook timeout enforced | Unit test: slow hook → HookTimeout | S9.4 |

### Agent Runner (S10)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S29 | Handshake sends 4 messages in order | Unit test: message sequence | S10.2 |
| S30 | Stderr not parsed as protocol JSON | Unit test: stderr line → no parse | S10.3 |
| S31 | User input request = hard failure | Unit test: input signal → immediate error | S10.5 |
| S32 | Unsupported tool call → failure result, session continues | Unit test | S10.5 |
| S33 | Read/turn/stall timeouts enforced | Unit test: timeout → specific error | S10.6 |

### Tracker (S11)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S34 | Linear query uses `slugId` filter | Unit/integration test: query shape | S11.2 |
| S35 | Issue state refresh uses `[ID!]` type | Unit test: query variable typing | S11.2 |
| S36 | Labels normalized to lowercase | Unit test: "BUG" → "bug" | S11.3 |
| S37 | Blockers from inverse "blocks" relation | Unit test: relation parsing | S11.3 |
| S38 | Empty ID list → no API call | Unit test: empty → immediate return | S11.1 |

### Observability (S13)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S39 | Issue logs include issue_id + issue_identifier | Log inspection | S13.1 |
| S40 | Session logs include session_id | Log inspection | S13.1 |
| S41 | Token totals use absolute values, not deltas | Unit test: accumulation logic | S13.5 |
| S42 | Sink failure does not crash orchestrator | Unit test: broken sink → continued | S13.2 |

### Service Hardness (S43-S48)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S43 | `/healthz` returns 200 always (liveness) | Unit test: healthz_returns_200 | S13.7 |
| S44 | `/readyz` returns 200 when initialized, 503 otherwise | Unit test: readyz tests | S13.7 |
| S45 | `symphony stop` triggers graceful shutdown | Integration test: POST /api/v1/shutdown | — |
| S46 | Bare `symphony` starts daemon (backward compat) | Unit test: None command → Start | — |
| S47 | Stalled sessions are killed and retried | Unit test: stall detection + abort handle | S8.5 |
| S48 | SIGTERM/SIGINT triggers clean shutdown with drain | Integration test: signal → drain → stop | — |
| S49 | Graceful drain waits for in-flight workers | Code review: drain loop in scheduler | — |
| S50 | Worker abort handles tracked and cleaned up | Code review: cleanup_worker_handles | — |

### CLI (S51-S56)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S51 | `symphony run STI-123` executes single issue without daemon | Unit test: run command parses | — |
| S52 | `--concurrency` overrides config `max_concurrent_agents` | Unit test: CLI override applied | — |
| S53 | `--tickets STI-1,STI-2` filters dispatch to named issues | Scheduler: ticket filter applied | — |
| S54 | `--once` runs single poll cycle then exits | Scheduler: once mode breaks loop | — |
| S55 | `--host` + `--token` enable remote daemon access | Client: build_client with URL + bearer | — |
| S56 | `GET /api/v1/metrics` returns usage data for metering | Unit test: endpoint returns JSON | — |
| S57 | `GET /metrics` returns Prometheus text format | Unit test: prometheus_metrics_returns_text | — |
| S58 | `/metrics` endpoint bypasses auth | Unit test: prometheus_metrics_bypasses_auth | — |

### PR Review Loop (S59-S62)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S59 | `pr_feedback` hook config parsed from WORKFLOW.md | Unit test: config extraction | S5.3.4 ext |
| S60 | `pr_feedback` hook runs after `after_run` between turns | Unit test: hook execution order | S9.4 ext |
| S61 | `pr_feedback` output captured and available as next-turn context | Unit test: feedback piped to turn | S7.1 ext |
| S62 | `pr_feedback` failure is logged and ignored (non-fatal) | Unit test: hook fail → no error | S9.4 ext |

### GitHub Tracker (S63-S68)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S63 | `tracker.kind: github` accepted by config validation | Unit test: validate_config_passes_with_github_kind | S6.3 |
| S64 | GitHub issues normalized to Symphony Issue type | Unit test: normalize_github_issue | S11.3 |
| S65 | Pull requests filtered from GitHub issues response | Unit test: normalize_filters_pull_requests | — |
| S66 | `project_slug` parsed as `owner/repo` | Unit test: parse_owner_repo_valid | — |
| S67 | Label-based state mapping for active states | Unit test: state_from_labels_matches_active | — |
| S68 | `create_tracker()` factory dispatches on kind | Code: start.rs + run.rs use factory | S11.1 |

### Done State Transition (S69-S70)
| ID | Setpoint | Measurement | Spec |
|----|----------|-------------|------|
| S69 | `done_state` transition called on normal worker exit | Code: dispatch_and_run calls set_issue_state after normal exit | S8.4 ext |
| S70 | `set_issue_state` failure is logged but doesn't block retry scheduling | Code: error logged with warn, handle_worker_exit still called | S8.4 ext |

---

## Sensors (How we measure)

| Sensor | Command | Frequency |
|--------|---------|-----------|
| Compile | `cargo check --workspace` | Every change |
| Lint | `cargo clippy --workspace -- -D warnings` | Every change |
| Test | `cargo test --workspace` | Every change |
| Smoke gate | `make smoke` | Before commit |
| Format check | `cargo fmt --all -- --check` | Before PR |
| Control audit | `make control-audit` | Before PR |
| Test count | `cargo test --workspace 2>&1 \| grep "test result"` | Per phase |
| Conformance | Review REQUIREMENTS.md checklist | Per phase |

---

## Controller Policy

```
ALWAYS:
  Run `make smoke` before committing
  Add tests that verify spec behavior for new code
  Reference spec section in test names or comments

IF smoke fails THEN:
  Fix errors before proceeding
  Do NOT skip or suppress warnings
  Do NOT add #[allow] without documenting justification

IF new code touches orchestrator state THEN:
  Verify single-authority mutation (no concurrent writes)
  Add test for idempotency if relevant

IF spec ambiguity found THEN:
  Document in .planning/research/<topic>.md
  Implement conservative interpretation
  Add test capturing expected behavior
  Note the decision in PLANS.md "Implementation-Defined Decisions"

IF phase completed THEN:
  Update STATE.md with test counts
  Update REQUIREMENTS.md checklist
  Run full `make control-audit`
  Verify all phase setpoints are green
```

---

## Actuator Map

| Actuator | Effect |
|----------|--------|
| `make smoke` | Gate: compile + lint + test |
| `make check` | Compile + lint only |
| `make test` | Tests only |
| `make build` | Release binary |
| `make control-audit` | Full audit: smoke + fmt check |
| `make install` | Install symphony binary to ~/.cargo/bin |
| `make publish-dry-run` | Verify all crates package for crates.io |
| `make publish` | Publish all crates to crates.io in dependency order |
| `cargo clippy --fix --allow-dirty` | Auto-fix lint issues |
| `cargo fmt --all` | Auto-format code |

---

## Deviation Log

| Date | Setpoint | Deviation | Resolution |
|------|----------|-----------|------------|
| 2026-03-06 | S2 | Clippy warnings on stub dead_code | Added `#[allow(dead_code)]` on stub structs (justified: fields used once impl complete) |
| 2026-03-06 | S2 | Clippy: manual_strip, collapsible_if, needless_borrows, derivable_impls, manual_map | Auto-fixed via `cargo clippy --fix` |
| 2026-03-06 | S10 | Rust 2024 edition: `set_var`/`remove_var` are unsafe | Wrapped in `unsafe` block in test (justified: single-threaded test context) |
| 2026-03-16 | S2 | Unused import `PathBuf` in workspace tests | Removed redundant import (already via `use super::*`) |
| 2026-03-16 | S2 | Clippy: too_many_arguments on Scheduler::new (8 args) | Added `#[allow(clippy::too_many_arguments)]` (justified: constructor groups related subsystems) |

---

## See Also

- [[docs/operations/Control Harness|Control Harness]] — build gates, test distribution, audit commands
- [[PLANS]] — implementation roadmap these setpoints verify
- [[.planning/REQUIREMENTS|Requirements]] — spec conformance checklist
- [[SPEC]] — canonical specification being verified
- [[AGENTS]] — agent guidelines referencing these controls
