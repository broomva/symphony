---
tags:
  - symphony
aliases:
  - README
created: 2026-03-06
---

# Symphony

> A Rust implementation of the [Symphony](https://github.com/openai/symphony) orchestration spec by OpenAI.
> For vault navigation see [[docs/Symphony Index|Symphony Index]]. For the canonical spec see [[SPEC]].

A Rust-based orchestration service that polls an issue tracker (Linear), creates isolated per-issue workspaces, and runs coding agent sessions automatically.

Symphony turns your issue backlog into autonomous coding work — it watches for "Todo" issues, clones your repo into a sandboxed workspace, runs a coding agent (like Claude Code), and manages retries, concurrency, and lifecycle hooks.

## How It Works

```
Linear (Todo issues)
       │
       ▼
   ┌────────────────────────┐
   │  Symphony Scheduler    │  ← polls every N seconds
   │  ┌──────────────────┐  │
   │  │ Fetch candidates │  │
   │  │ Select & sort    │  │
   │  │ Dispatch workers │  │
   │  └──────────────────┘  │
   └────────────┬───────────┘
                │
       ┌────────┴────────┐
       ▼                 ▼
  ┌──────────┐     ┌──────────┐
  │ Worker 1 │     │ Worker N │    ← one per issue
  │ workspace│     │ workspace│
  │ + agent  │     │ + agent  │
  └──────────┘     └──────────┘
                │
                ▼
   ┌─────────────────────┐
   │  HTTP Dashboard     │  ← live state at :8080
   │  /api/v1/state      │
   │  /api/v1/refresh    │
   └─────────────────────┘
```

**Poll loop**: Fetches active issues from Linear → filters by project & state → dispatches up to `max_concurrent_agents` workers.

**Per-issue worker**: Creates workspace directory → runs lifecycle hooks (clone repo, rebase, etc.) → renders prompt template with issue data → launches coding agent → runs post-hooks (commit, etc.).

**Reconciliation**: On each tick, refreshes running issue states from Linear. If an issue moves to a terminal state (Done/Canceled), the worker is cleaned up.

## Quick Start

### Prerequisites

- Rust 1.85+ (edition 2024)
- A [Linear](https://linear.app) API key
- A coding agent CLI (e.g., `claude`)
- `gh` CLI (if using GitHub hooks)

### Build

```bash
make build    # release build → target/release/symphony
```

### Configure

Create a `WORKFLOW.md` in your project root. This file has YAML frontmatter (config) and a Liquid template body (the prompt sent to the agent):

```markdown
---
tracker:
  kind: linear
  api_key: $LINEAR_API_KEY          # env var expansion
  project_slug: 71c211385593        # Linear project slug ID
  active_states:
    - Todo
  terminal_states:
    - Done
    - Canceled
    - Duplicate
polling:
  interval_ms: 30000                # poll every 30s
workspace:
  root: ~/my-workspaces/project     # per-issue dirs created here
hooks:
  after_create: |                   # runs once when workspace is first created
    gh repo clone MyOrg/my-repo . -- --depth 50
    git checkout -b "$SYMPHONY_ISSUE_ID"
  before_run: |                     # runs before each agent session
    git fetch origin main
    git rebase origin/main || git rebase --abort
  after_run: |                      # runs after each agent session (failure ignored)
    git add -A
    git diff --cached --quiet || git commit -m "$SYMPHONY_ISSUE_ID: automated changes"
  timeout_ms: 120000
agent:
  max_concurrent_agents: 1
  max_turns: 10
codex:
  command: "claude --dangerously-skip-permissions"
server:
  port: 8080
---
You are a senior software engineer.

## Task
{{ issue.identifier }}: {{ issue.title }}

{% if issue.description %}
## Description
{{ issue.description }}
{% endif %}

{% if issue.labels %}
## Labels
{{ issue.labels | join: ", " }}
{% endif %}

## Instructions
- Read the codebase thoroughly before making changes
- Write clean, well-tested code following existing patterns
- Run existing tests to make sure nothing is broken

{% if attempt %}
## Retry
This is retry attempt {{ attempt }}. The previous attempt failed.
Review what went wrong and try a different approach.
{% endif %}
```

### Set your API key

```bash
export LINEAR_API_KEY="lin_api_..."
```

### Run

```bash
# Use default WORKFLOW.md in current directory
symphony

# Or specify a path
symphony /path/to/WORKFLOW.md

# Override the dashboard port
symphony --port 9090
```

Symphony will start polling Linear, and you'll see structured logs:

```
symphony starting                    workflow="WORKFLOW.md"
startup terminal cleanup complete    cleaned=2
fetched candidate issues             count=7
dispatching issue                    identifier="STI-746" title="Add auth flow"
workspace created                    path="~/my-workspaces/project/STI-746"
agent session completed              identifier="STI-746" exit_code=0
```

### Dashboard

Open `http://localhost:8080` for a live HTML dashboard showing running/retrying issues, token usage, and concurrency.

**API endpoints:**

| Endpoint | Description |
|----------|-------------|
| `GET /` | HTML dashboard |
| `GET /api/v1/state` | Full orchestrator state JSON |
| `GET /api/v1/issues/:id` | Single issue status |
| `POST /api/v1/refresh` | Trigger immediate poll |
| `GET /health` | Health check |

## Architecture

Rust workspace with 7 crates:

| Crate | Responsibility |
|-------|---------------|
| `symphony-core` | Domain types: Issue, Session, Workspace, OrchestratorState |
| `symphony-config` | WORKFLOW.md loader, typed config, live file watcher |
| `symphony-tracker` | Linear GraphQL client, issue fetching, state normalization |
| `symphony-workspace` | Per-issue directory lifecycle, hook execution, path safety |
| `symphony-agent` | Coding agent subprocess management (CLI pipe + JSON-RPC modes) |
| `symphony-orchestrator` | Poll loop, dispatch, reconciliation, retry queue |
| `symphony-observability` | Structured logging, HTTP dashboard + REST API |

### Key Features

- **Live config reload**: Edit `WORKFLOW.md` while running — changes apply on next tick
- **Lifecycle hooks**: `after_create`, `before_run`, `after_run`, `before_remove` with timeout enforcement
- **Retry with backoff**: Failed sessions retry with exponential backoff; continuations retry immediately
- **Concurrency control**: Configurable `max_concurrent_agents` with slot-based dispatch
- **Workspace isolation**: Each issue gets its own directory; path traversal attacks are blocked
- **Template rendering**: Liquid templates with full issue context (title, description, labels, attempt count)
- **Environment injection**: `$SYMPHONY_ISSUE_ID` available in all hook scripts
- **Terminal cleanup**: On startup, cleans workspaces for issues already in Done/Canceled state

## WORKFLOW.md Reference

### Frontmatter Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `tracker.kind` | yes | — | Tracker type (`linear`) |
| `tracker.api_key` | yes | — | API key (supports `$ENV_VAR` syntax) |
| `tracker.project_slug` | yes | — | Linear project slug ID |
| `tracker.active_states` | no | `["Todo"]` | States that make an issue eligible |
| `tracker.terminal_states` | no | `["Done","Canceled"]` | States that trigger cleanup |
| `polling.interval_ms` | no | `30000` | Poll interval in milliseconds |
| `workspace.root` | no | `./workspaces` | Root directory for per-issue workspaces |
| `hooks.after_create` | no | — | Shell script run once on new workspace |
| `hooks.before_run` | no | — | Shell script run before each agent session |
| `hooks.after_run` | no | — | Shell script run after each agent session |
| `hooks.before_remove` | no | — | Shell script run before workspace cleanup |
| `hooks.timeout_ms` | no | `30000` | Hook execution timeout |
| `agent.max_concurrent_agents` | no | `1` | Max parallel agent sessions |
| `agent.max_turns` | no | `10` | Max agent turns per session |
| `codex.command` | yes | — | Agent CLI command to run |
| `server.port` | no | — | HTTP dashboard port (omit to disable) |

### Template Variables

The body after the `---` frontmatter is a [Liquid](https://shopify.github.io/liquid/) template with these variables:

| Variable | Type | Description |
|----------|------|-------------|
| `issue.identifier` | string | Issue ID (e.g., `STI-746`) |
| `issue.title` | string | Issue title |
| `issue.description` | string | Issue body/description |
| `issue.labels` | array | Issue labels |
| `attempt` | number | Retry attempt number (nil on first run) |

## Development

```bash
make smoke    # compile + clippy + test (the gate)
make check    # cargo check + clippy
make test     # cargo test --workspace
make build    # cargo build --release
make fmt      # cargo fmt --all
```

136 tests across all crates (131 unit + 5 integration tests requiring `LINEAR_API_KEY`).

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Key extension points:
- **Tracker plugins**: Implement the `TrackerClient` trait to add support for GitHub Issues, Jira, etc.
- **Agent runners**: The agent runner supports any CLI that speaks line-delimited JSON on stdout
- **Workflow templates**: Create new `WORKFLOW.md` examples for different use cases

## Community

- [Issues](https://github.com/broomva/symphony/issues) — bug reports, feature requests
- [Discussions](https://github.com/broomva/symphony/discussions) — questions, ideas, show & tell

## License

Apache License 2.0 — see [LICENSE](LICENSE) for details.

This project implements the [Symphony specification](https://github.com/openai/symphony) originally published by OpenAI under the Apache 2.0 license.
