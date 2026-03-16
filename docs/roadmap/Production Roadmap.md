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

Must-have before any deployment beyond localhost.

| Area | Gap | Why | Phase |
|------|-----|-----|-------|
| **Graceful shutdown** | No SIGTERM/SIGINT handling | K8s/Docker sends SIGTERM; orphaned workers | 8 |
| **Health endpoints** | No `/healthz` or `/readyz` | Load balancer + orchestrator readiness | 8 |
| **Stall kill** | Detected not acted on | Zombie agent processes accumulate | Core gap |
| **Graceful drain** | No "shutting down, let runs finish" | Deploys kill in-flight work | 8 |

## Tier 2 — Operability (production confidence)

| Area | Gap | Why | Phase |
|------|-----|-----|-------|
| **Docker** | No Dockerfile/Compose | Deployment story | [[PLANS]] 8.3 |
| **CI/CD** | No GitHub Actions | Automated gate enforcement | [[PLANS]] 8.2 |
| **Prometheus** | No `/metrics` endpoint | Standard observability stack | Post-8 |
| **Env config** | Only WORKFLOW.md | Managed services use env/secrets | Post-8 |
| **Examples** | No example workflows | Onboarding for new users | [[PLANS]] 8.4 |

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
