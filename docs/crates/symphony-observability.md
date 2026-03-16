---
tags:
  - symphony
  - crate
  - observability
  - http
created: 2026-03-16
---

# symphony-observability

**Spec coverage**: S13 (Logging, Status, Observability)
**Path**: `crates/symphony-observability/src/`
**Tests**: 5

HTTP server with HTML dashboard and JSON API for runtime monitoring.

## Source Files

| File | Lines | Purpose |
|------|-------|---------|
| `server.rs` | 434 | Axum router, dashboard, JSON API endpoints, state building |
| `lib.rs` | 23 | Tracing initialization, module exports |

## HTTP Endpoints

| Method | Path | Response | Spec |
|--------|------|----------|------|
| GET | `/` | HTML dashboard | S13.7.1 |
| GET | `/api/v1/state` | JSON system summary | S13.7.2 |
| GET | `/api/v1/{identifier}` | JSON issue detail (404 if unknown) | S13.7.2 |
| POST | `/api/v1/refresh` | 202 Accepted, triggers immediate poll | S13.7.2 |
| * | * | 405 Method Not Allowed | S13.7.2 |

Error envelope: `{"error": {"code": "...", "message": "..."}}`

## Server Enablement (S13.7)

- Start when CLI `--port` provided OR `server.port` in [[WORKFLOW]]
- Precedence: CLI `--port` overrides config
- Binds loopback `127.0.0.1` by default
- Port `0` = ephemeral (OS-assigned)

## Dashboard

Server-rendered HTML showing:
- Running agent count and list
- Retrying issues with delay info
- Token totals (input/output/total)
- Cumulative runtime (seconds)

## State Snapshot (S13.3)

Returns at query time:
- `running`: list with turn_count per issue
- `retrying`: list with next retry time
- `codex_totals`: input/output/total tokens, seconds_running
- Active session elapsed derived from `started_at`

## See Also

- [[docs/operations/Control Harness|Control Harness]] — operational monitoring
- [[docs/roadmap/Production Roadmap|Production Roadmap]] — Prometheus metrics, auth, WebSocket planned
