---
tags:
  - symphony
  - architecture
  - decision
type: decision
status: active
area: business
aliases:
  - Architecture Decision
  - Open Core Model
created: 2026-03-06
---

# Architecture Decision: Open Core Model

> [!info] Related
> See [[docs/roadmap/Production Roadmap|Production Roadmap]] for the path to managed service and [[PLANS|Phase 9]] for the Symphony Cloud implementation plan.

## Context

Symphony implements the [[SPEC|Symphony orchestration spec]] (Apache 2.0, OpenAI) as a Rust-based engine. We want to:
1. Build a healthy open-source community around the engine
2. Offer a managed service (SaaS) for teams who don't want to self-host
3. Protect the intellectual property that makes the managed service valuable

## Decision: Two-Repo Open Core

### Public: `symphony` (Apache 2.0)

The orchestration engine — everything needed to self-host.

```
symphony/
├── crates/
│   ├── symphony-core/         # Domain model (Issue, Session, Workspace)
│   ├── symphony-config/       # WORKFLOW.md parsing, live reload
│   ├── symphony-tracker/      # Linear client (+ community: GitHub, Jira)
│   ├── symphony-workspace/    # Per-issue workspace lifecycle, hooks
│   ├── symphony-agent/        # Agent subprocess runner (Claude, Codex, etc.)
│   ├── symphony-orchestrator/ # Poll loop, dispatch, retry, reconciliation
│   └── symphony-observability/# Logging, basic HTTP dashboard + API
├── src/main.rs                # CLI binary
├── examples/                  # Example WORKFLOW.md files
├── Dockerfile
└── LICENSE                    # Apache 2.0
```

**What stays open**: engine, CLI, basic dashboard, all tracker/agent plugins, WORKFLOW.md format, Docker support.

**Why Apache 2.0**: Matches the upstream spec license. Maximizes adoption. The engine is not the moat — the managed platform is.

### Private: `symphony-cloud` (Proprietary)

The managed service platform — everything beyond self-hosting.

```
symphony-cloud/                # next-forge monorepo
├── apps/
│   ├── web/                   # Marketing site
│   ├── app/                   # Dashboard (auth, billing, tenant mgmt)
│   ├── api/                   # Control plane API
│   └── desktop/               # Tauri desktop app (later)
├── packages/
│   ├── ui/                    # Shared design system
│   ├── db/                    # Tenant configs, run history, audit logs
│   ├── symphony-client/       # TypeScript SDK for Symphony HTTP API
│   ├── auth/                  # Multi-tenant authentication (Clerk)
│   ├── billing/               # Subscriptions and usage metering (Stripe)
│   └── analytics/             # Usage tracking
└── infra/                     # Deployment, provisioning
```

**What stays private**: multi-tenancy, auth, billing, enhanced dashboard, control plane, infrastructure, desktop app.

**Why private**: These features represent significant engineering effort beyond the spec. They are the differentiation for the managed service.

## The Line

| Component | Open | Private | Rationale |
|-----------|------|---------|-----------|
| Orchestrator engine | X | | Spec implementation, community value |
| CLI binary | X | | Adoption driver |
| Basic HTML dashboard | X | | Self-hosters need it |
| WORKFLOW.md format | X | | Open standard |
| Tracker plugins | X | | Community contributions |
| Agent runners | X | | Community contributions |
| Docker/deployment | X | | Self-hosting support |
| Multi-tenant control plane | | X | Core SaaS infrastructure |
| Enhanced dashboard | | X | Differentiated UX |
| Auth + team management | | X | Enterprise feature |
| Billing + usage metering | | X | Monetization |
| Run history + analytics | | X | Data platform |
| Desktop app | | X | Connects to cloud |

## Precedents

This model is used by:
- **Supabase**: PostgreSQL (open) + dashboard/auth/realtime (open core)
- **GitLab**: CE (open) + EE features (proprietary)
- **Grafana**: AGPL core + enterprise plugins (proprietary)
- **PostHog**: Open core analytics + cloud features
- **Cal.com**: AGPL core + enterprise features

## Monetization

| Tier | Price | Limits |
|------|-------|--------|
| Self-hosted | Free | Unlimited (bring your own infra) |
| Starter | $49/mo | 3 concurrent agents, 1 project |
| Team | $199/mo | 10 agents, unlimited projects, team auth |
| Enterprise | Custom | Unlimited, SSO, SLA, dedicated infra |

## Future Considerations

- If community demand is strong, consider open-sourcing the desktop app
- Keep the TypeScript SDK (`symphony-client`) as a candidate for open-sourcing — it drives adoption
- The enhanced dashboard could become open-source once the control plane is the primary moat

## See Also

- [[PLANS]] — Phase 8 (OSS release) and Phase 9 (Symphony Cloud) detail the execution plan
- [[docs/roadmap/Production Roadmap|Production Roadmap]] — technical hardening required before launch
- [[CONTRIBUTING]] — contributor guide for the open engine
- [[docs/architecture/Crate Map|Crate Map]] — what ships in the open repo
