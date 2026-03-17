---
tags:
  - symphony
  - operations
  - observability
type: operations
status: active
area: observability
aliases:
  - Observability
  - Observability Runbook
created: 2026-03-17
---

# Observability

Centralized runbook for Symphony's observability stack: structured logging, HTTP API, Prometheus metrics, health checks, and token accounting.

> [!info] Implementation
> The observability crate lives at `crates/symphony-observability/`. See [[docs/crates/symphony-observability|symphony-observability]] for API details.

## Structured Logging

Symphony uses `tracing` with JSON output for machine-readable logs.

### Required Span Fields

| Field | Context | Source |
|-------|---------|--------|
| `issue_id` | Issue processing | Tracker response |
| `issue_identifier` | Issue processing | Tracker response (e.g., `STI-123`) |
| `session_id` | Agent execution | `<thread_id>-<turn_id>` |
| `workspace_key` | Workspace ops | Sanitized issue identifier |

### Log Levels

| Level | Use For |
|-------|---------|
| `ERROR` | Unrecoverable failures, hook timeouts |
| `WARN` | Retriable failures, state transition issues, sink errors |
| `INFO` | Lifecycle events: poll, dispatch, session start/end |
| `DEBUG` | Protocol messages, config reload, template rendering |
| `TRACE` | Raw HTTP, GraphQL queries |

## HTTP Endpoints

All endpoints served by `symphony-observability` on the configured port (default: 8080).

| Endpoint | Method | Auth | Purpose |
|----------|--------|------|---------|
| `/` | GET | No | HTML dashboard with live state |
| `/healthz` | GET | No | Liveness probe — always 200 |
| `/readyz` | GET | No | Readiness probe — 200 when initialized, 503 otherwise |
| `/metrics` | GET | No | Prometheus text format metrics |
| `/api/v1/state` | GET | Bearer | Full orchestrator state as JSON |
| `/api/v1/workspaces` | GET | Bearer | Active workspace listing |
| `/api/v1/metrics` | GET | Bearer | Usage metrics for metering (JSON) |
| `/api/v1/{identifier}` | GET | Bearer | Single issue detail by identifier |
| `/api/v1/shutdown` | POST | Bearer | Trigger graceful shutdown |

### Authentication

Bearer token via `SYMPHONY_API_TOKEN` env var. When set, all `/api/v1/*` endpoints require `Authorization: Bearer <token>`. Health and metrics endpoints are always open.

## Prometheus Metrics

Exposed at `/metrics` in Prometheus text format:

| Metric | Type | Description |
|--------|------|-------------|
| `symphony_issues_total` | Gauge | Total tracked issues |
| `symphony_issues_active` | Gauge | Issues currently being worked |
| `symphony_sessions_total` | Counter | Total agent sessions started |
| `symphony_sessions_running` | Gauge | Currently running sessions |
| `symphony_sessions_completed` | Counter | Successfully completed sessions |
| `symphony_sessions_failed` | Counter | Failed sessions (all causes) |
| `symphony_tokens_input_total` | Counter | Cumulative input tokens (absolute) |
| `symphony_tokens_output_total` | Counter | Cumulative output tokens (absolute) |
| `symphony_poll_duration_seconds` | Histogram | Tracker poll latency |
| `symphony_hook_duration_seconds` | Histogram | Hook execution latency |

> [!important] Token Accounting
> Token totals use **absolute values** from the API, not deltas. Each session report overwrites the running total for that session. See setpoint S41 in [[CONTROL]].

## Health Checks

### Liveness (`/healthz`)

Always returns 200. Use for container orchestrator liveness probes (Kubernetes, Railway).

### Readiness (`/readyz`)

Returns 200 once the orchestrator has completed initialization (config loaded, tracker connected). Returns 503 during startup. Use for load balancer readiness gates.

## Dashboard

The HTML dashboard at `/` provides:
- Orchestrator status (running/draining/stopped)
- Active issues with state, priority, attempt count
- Recent session history with exit codes
- Auto-refresh every 30 seconds

## Configuration

See [[docs/operations/Configuration Reference|Configuration Reference]] for all settings. Key observability settings:

```yaml
server:
  port: 8080  # HTTP server port
```

Environment:
- `SYMPHONY_API_TOKEN` — Bearer auth for API endpoints (optional)
- `RUST_LOG` — Tracing filter (default: `info`)

## See Also

- [[docs/crates/symphony-observability|symphony-observability]] — Crate implementation details
- [[docs/operations/Configuration Reference|Configuration Reference]] — Full WORKFLOW.md format
- [[CONTROL]] — Setpoints S39-S42 (observability), S43-S50 (service hardness), S56-S58 (metrics)
- [[docs/operations/Control Harness|Control Harness]] — Build gates and audit commands
