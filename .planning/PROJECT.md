# PROJECT.md - Symphony

## Vision
A production-quality Rust implementation of the Symphony Service Specification —
a long-running daemon that orchestrates coding agents to get project work done
by polling Linear for issues, creating isolated workspaces, and running
Codex app-server sessions.

## Source of Truth
- Canonical spec: `/Users/broomva/Downloads/Symphony SPEC.md` (Draft v1)
- Architecture: `AGENTS.md`
- Roadmap: `PLANS.md`

## Tech Stack
- **Language**: Rust (edition 2024)
- **Async runtime**: Tokio
- **HTTP client**: reqwest
- **HTTP server**: Axum (optional dashboard)
- **Templating**: Liquid (prompt rendering)
- **Config format**: YAML front matter in WORKFLOW.md
- **File watching**: notify
- **Logging**: tracing + tracing-subscriber (JSON)
- **CLI**: clap

## Key Constraints
- In-memory orchestrator state (no DB required)
- Dynamic config reload without restart
- Single-authority state mutations (no concurrent writes)
- Workspace path containment safety invariant
- Strict template rendering (unknown vars = error)

## Delivery Style
- Layered workspace crates matching spec abstraction levels
- Phase-based implementation following PLANS.md
- Each phase ends with `make smoke` passing
- Harness engineering: AGENTS.md + PLANS.md + Makefile gates
