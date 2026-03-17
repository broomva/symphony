---
tags:
  - symphony
  - control
  - meta
type: reference
status: active
area: development
aliases:
  - Metalayer
  - Control Metalayer Reference
created: 2026-03-17
---

# METALAYER.md — Canonical Control Loop Definition

> [!important] Machine-readable companion to CONTROL.md
> This document defines the control metalayer architecture. For human-readable setpoints, see [[CONTROL]]. For the machine-readable representations, see `.control/`.

## Purpose

The control metalayer provides **stability across agent sessions**. Every agent — human or AI — reads the same setpoints, runs the same sensors, and produces verifiably correct output. The `.control/` directory makes this machine-readable so scripts and agents can programmatically verify compliance.

## Architecture

### `.control/` Directory

| File | Purpose | Updated By |
|------|---------|-----------|
| `policy.yaml` | All 76 setpoints with ID, category, measurement, severity | Manual (mirrors CONTROL.md) |
| `commands.yaml` | Makefile targets typed as gates, sensors, actuators | Manual (mirrors Makefile) |
| `topology.yaml` | 8 crates with path, spec layer, dependencies, test count | Manual (mirrors Cargo.toml) |
| `state.json` | Live metric snapshot: version, tests, gate status | `scripts/control/refresh_state.sh` |

### Validation Scripts

| Script | Purpose |
|--------|---------|
| `scripts/control/refresh_state.sh` | Regenerate `state.json` from live measurements |
| `scripts/control/validate_policy.sh` | Cross-check policy.yaml IDs against CONTROL.md |

## Control Loop

The development control loop operates at three levels:

```
┌─────────────────────────────────────────────┐
│              SENSE (Sensors)                 │
│  cargo check, cargo test, cargo clippy      │
│  harness-audit, entropy-check               │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              PLAN (Controller Policy)        │
│  Read setpoints → identify affected ones     │
│  Check state.json → current metric snapshot  │
│  Decide: implement, fix, or defer           │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              ACT (Actuators)                 │
│  Write code, run fmt, commit, push          │
│  Update CONTROL.md if new behavior added    │
│  Update state.json via refresh_state.sh     │
└──────────────────┬──────────────────────────┘
                   │
                   └──────── feedback ────────→ SENSE
```

### Per-Change Loop (Inner)

1. **CHECK** — Which setpoints does this change affect?
2. **IMPLEMENT** — Write code satisfying those setpoints
3. **MEASURE** — `make smoke` / `make control-audit`
4. **VERIFY** — All affected setpoints green?
5. **DOCUMENT** — Update CONTROL.md, state.json, Project Status
6. **FEEDBACK** — Deviation log if any setpoint was relaxed

### Per-Session Loop (Outer)

1. Read `.control/state.json` — understand current state
2. Read `.control/policy.yaml` — understand all setpoints
3. Work through changes using the inner loop
4. Run `make control-refresh` — update state.json
5. Run `make control-validate` — verify policy alignment

## Profiles

| Profile | Description | Use Case |
|---------|-------------|----------|
| **Baseline** | All setpoints enforced, manual review | Default for development |
| **Governed** | All setpoints enforced, automated gates | CI/CD pipeline |
| **Autonomous** | Blocking setpoints enforced, informational logged | Agent-driven development |

## State Management

`state.json` is the single source of truth for **current** project metrics. It is:
- **Generated**, not manually edited (via `refresh_state.sh`)
- **Versioned** in git for audit trail
- **Queried** by agents and scripts to understand project health

## Integration

### With CONTROL.md
`policy.yaml` mirrors CONTROL.md setpoints. `validate_policy.sh` ensures they stay in sync. If you add a setpoint to CONTROL.md, add it to policy.yaml too.

### With Makefile
`commands.yaml` mirrors Makefile targets. New targets should be added to both.

### With CI/CD
The `.control/` directory enables CI to:
- Validate all blocking setpoints pass
- Compare test counts against recorded state
- Detect setpoint drift between CONTROL.md and policy.yaml

## See Also

- [[CONTROL]] — Human-readable setpoints (source of truth)
- [[docs/operations/Control Harness|Control Harness]] — Build gates and audit commands
- [[AGENTS]] — Agent guidelines referencing the control loop
- [[CLAUDE]] — Development conventions including control metalayer section
