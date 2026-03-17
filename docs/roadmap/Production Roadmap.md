---
tags:
  - symphony
  - roadmap
  - production
type: roadmap
status: active
area: project
created: 2026-03-16
---

# Production Roadmap

Path from working orchestrator to fully managed service. See [[PLANS]] Phase 8 and Phase 9 for detailed task breakdowns.

## Tier 1 — Service Hardness (before exposing)

All Tier 1 gaps resolved.

| Area | Status | Resolution |
|------|--------|------------|
| **Graceful shutdown** | Done | SIGTERM/SIGINT handler → watch channel → scheduler + HTTP server |
| **Health endpoints** | Done | `/healthz` (liveness, always 200), `/readyz` (readiness, 503 until initialized) |
| **Stall kill** | Done | Abort handles tracked per worker; stalled sessions killed + retried with backoff |
| **Graceful drain** | Done | Scheduler drain loop: stops dispatch, waits for in-flight workers to complete |

## Tier 2 — Operability (production confidence)

| Area | Gap | Why | Phase |
|------|-----|-----|-------|
| **Docker** | Done | Multi-stage Dockerfile + docker-compose.yml with healthcheck | [[PLANS]] 8.3 |
| **CI/CD** | Done | GitHub Actions: check, test, build (3 targets), docker build | [[PLANS]] 8.2 |
| **Examples** | Done | 3 example workflows: linear-claude, linear-codex, github-claude | [[PLANS]] 8.4 |
| **Prometheus** | Planned | No `/metrics` endpoint yet | Post-8 |
| **Env config** | Planned | Only WORKFLOW.md; managed services prefer env/secrets | Post-8 |

## Tier 3 — Open Source Release ([[PLANS]] Phase 8)

| Task | Description |
|------|-------------|
| 8.1 | License: Apache 2.0, NOTICE file |
| 8.2 | CI/CD: GitHub Actions, release binaries, crates.io, ghcr.io |
| 8.3 | Docker: multi-stage build, compose example |
| 8.4 | Example workflows: Linear+Claude, Linear+Codex, GitHub+Claude |
| 8.5 | Contributing guide, CoC, issue templates |
| 8.6 | Plugin architecture docs: tracker + agent runner extension |

## Tier 4 — Symphony Cloud ([[PLANS]] Phase 9)

Full managed service with multi-tenancy.

| Task | Description |
|------|-------------|
| 9.1 | Scaffold next-forge monorepo (web, app, api) |
| 9.2 | TypeScript client SDK for Symphony HTTP API |
| 9.3 | Dashboard MVP: real-time agents, logs, workflow editor |
| 9.4 | Control plane API: tenant provisioning, workflow CRUD, secrets |
| 9.5 | Auth (Clerk) + multi-tenancy + RBAC |
| 9.6 | Billing (Stripe): agent-hours, tokens, concurrent slots |
| 9.7 | Infrastructure: per-tenant containers, auto-scaling, health monitoring |
| 9.8 | Desktop app (Tauri v2, optional) |

## Recommended Sequencing

```
Current ──► Tier 1 (hardness) ──► Tier 2 (ops) ──► Phase 8 (OSS) ──► Phase 9 (Cloud)
  │              │                     │                  │                   │
  │         graceful shutdown      Docker/CI         license/examples    next-forge
  │         health checks          metrics           plugin docs         dashboard
  │         stall kill             env config        contributing        billing
  │         worker drain                                                 multi-tenant
  ▼
 NOW
```

## See Also

- [[PLANS]] — detailed task definitions for Phases 8-9
- [[docs/roadmap/Project Status|Project Status]] — current state
- [[CONTROL]] — control harness for quality gates
