---
tags:
  - symphony
  - architecture
  - crate
type: architecture
status: active
area: system
created: 2026-03-16
---

# Crate Map

All crates live under `crates/` in the workspace. See [[docs/architecture/Architecture Overview|Architecture Overview]] for how they connect.

## Overview

| Crate | Spec | Lines | Tests | Status | Detail |
|-------|------|-------|-------|--------|--------|
| `symphony-core` | S4 | ~225 | 4 | Complete | [[docs/crates/symphony-core\|Detail]] |
| `symphony-config` | S5-6 | ~1,184 | 36 | Complete | [[docs/crates/symphony-config\|Detail]] |
| `symphony-tracker` | S11 | ~1,143 | 30 | Complete | [[docs/crates/symphony-tracker\|Detail]] |
| `symphony-workspace` | S9 | ~512 | 18 | Complete | [[docs/crates/symphony-workspace\|Detail]] |
| `symphony-agent` | S10 | ~1,128 | 16 | Complete | [[docs/crates/symphony-agent\|Detail]] |
| `symphony-orchestrator` | S7-8 | ~1,550 | 33 | Complete | [[docs/crates/symphony-orchestrator\|Detail]] |
| `symphony-observability` | S13 | ~457 | 5 | Complete | [[docs/crates/symphony-observability\|Detail]] |
| `symphony` (root) | S17.7 | ~202 | 5 | Complete | `src/main.rs` |
| **Total** | | **~6,093** | **136** | | +5 opt-in |

## Dependency Graph

```
symphony (bin)
  ├── symphony-orchestrator
  │     ├── symphony-core
  │     ├── symphony-config         ← HiveConfig lives here
  │     ├── symphony-tracker
  │     ├── symphony-workspace
  │     ├── symphony-agent
  │     └── symphony-arcan          ← Arcan runtime adapter
  ├── symphony-arcan
  │     └── symphony-core
  ├── symphony-observability
  │     └── symphony-core
  └── symphony-config
```

**Hive mode cross-crate dependencies** (external, not in this workspace):
- `aios-protocol` — HiveTaskId + 5 Hive EventKind variants
- `lago-core` — EventQuery metadata/kind filters + HiveTask aggregate
- `arcan-spaces` — HiveSpacesCoordinator for agent coordination
- `autoany-core` / `autoany-lago` — EGRI inject_history + replay_hive_history

## Conventions

Per [[CLAUDE]] and [[AGENTS]]:

- Rust edition 2024, minimum rustc 1.85
- `thiserror` for library errors, `anyhow` for application errors
- `tracing` for structured logging (never `println!` or `log`)
- `tokio` for async runtime
- `serde` for all serialization
- Tests in `#[cfg(test)] mod tests` at bottom of each file
