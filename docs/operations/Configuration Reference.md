---
tags:
  - symphony
  - operations
  - config
type: operations
status: active
area: config
created: 2026-03-16
---

# Configuration Reference

Symphony is configured via a [[WORKFLOW]] file (default: `./WORKFLOW.md`). The file uses YAML front matter for settings and Markdown body for the prompt template.

## File Format

```markdown
---
tracker:
  kind: linear
  api_key: $LINEAR_API_KEY
  # ... more settings
---
Your prompt template here with {{ issue.identifier }}
```

Front matter must be a YAML mapping. Non-map → `workflow_front_matter_not_a_map` error.

## Configuration Sections

### `tracker` (S5.3.1)

| Key | Type | Required | Default | Notes |
|-----|------|----------|---------|-------|
| `kind` | string | Yes | — | `"linear"`, `"github"`, or `"markdown"` |
| `endpoint` | string | No | `https://api.linear.app/graphql` | API endpoint; for markdown: optional Lago HTTP URL |
| `api_key` | string | Yes (linear/github) | — | Supports `$VAR` env resolution; not required for markdown |
| `project_slug` | string | Yes | — | Linear slug, `owner/repo` for GitHub, or directory path for markdown |
| `active_states` | list/CSV | No | `["Todo"]` | States to poll for |
| `terminal_states` | list/CSV | No | `["Done", "Canceled"]` | States that end work |
| `done_state` | string | No | — | Auto-transition issues to this state on agent success |

### `polling` (S5.3.2)

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `interval_ms` | integer | 30000 | String-integer coercion supported |

### `workspace` (S5.3.3)

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `root` | string | `./workspaces` | Supports `~` and `$VAR` expansion |

### `hooks` (S5.3.4)

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `after_create` | string | — | Shell script; failure = fatal |
| `before_run` | string | — | Shell script; failure = abort attempt |
| `after_run` | string | — | Shell script; failure = ignored |
| `before_remove` | string | — | Shell script; failure = ignored |
| `timeout_ms` | integer | 60000 | Non-positive → default |

All hooks run via `sh -lc <script>` with workspace as cwd. See [[docs/crates/symphony-workspace|workspace]].

### `agent` (S5.3.5)

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `max_concurrent_agents` | integer | 5 | Global concurrency cap |
| `max_turns` | integer | 10 | Turns per issue before stop |
| `max_retry_backoff_ms` | integer | 300000 | 5min cap on exponential backoff |
| `max_concurrent_agents_by_state` | map | — | Per-state limits; keys normalized |

### `codex` (S5.3.6)

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `command` | string | Required | Agent command to run |
| `approval_policy` | string | `"auto-edit"` | Auto-approve posture |
| `thread_sandbox` | string | `"none"` | |
| `turn_sandbox_policy` | string | `"none"` | |
| `turn_timeout_ms` | integer | 600000 | 10min total turn |
| `read_timeout_ms` | integer | 30000 | Handshake/sync timeout |
| `stall_timeout_ms` | integer | 300000 | 5min inactivity (0 = disabled) |

### `hive` (extension — multi-agent collaborative evolution)

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `enabled` | bool | `false` | Enable hive mode for issues with `hive` label |
| `agents_per_task` | integer | 3 | Concurrent agents per hive task |
| `max_generations` | integer | 5 | Stop after N generations |
| `convergence_threshold` | float | 0.01 | Stop if score improvement < threshold |
| `egri_budget_per_agent` | integer | 10 | EGRI trial budget per agent per generation |
| `eval_script` | string | — | Optional evaluation script path |
| `spaces_server_id` | integer | — | Spaces server ID for coordination channels |

Issues with the `hive` label trigger multi-agent dispatch when `hive.enabled: true`. All other issues use the single-agent path unchanged.

### `server` (extension)

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `port` | integer | — | CLI `--port` overrides this |

## Environment Variable Resolution

- `$VAR_NAME` in `api_key` and path values → `env::var("VAR_NAME")`
- Unset/empty → empty string (treated as missing for validation)

## Dynamic Reload (S6.2)

- File watcher detects WORKFLOW.md changes (create/modify/remove)
- On change: re-read → re-parse → re-apply config + prompt template
- Invalid reload: keep last known good config, emit error
- In-flight sessions NOT restarted
- Applied to: polling cadence, concurrency limits, states, codex settings, hooks, prompts

## Dispatch Validation (S6.3)

Before each tick, validates:
- `tracker.kind` present and supported (`"linear"`, `"github"`, or `"markdown"`)
- `tracker.api_key` non-empty after `$VAR` resolution (linear/github only)
- `tracker.project_slug` present
- `codex.command` non-empty

Failure → skip dispatch for that tick, keep reconciliation running.

## Prompt Template

Markdown body after front matter. Uses Liquid syntax:
- `{{ issue.identifier }}`, `{{ issue.title }}`, `{{ issue.description }}`
- `{{ issue.labels | join: ", " }}`, `{{ issue.labels | size }}`
- `{{ attempt }}` — null on first run, integer on retry
- `{% if issue.description %}...{% endif %}` — conditionals
- Empty body → fallback: "You are working on an issue from Linear."

## See Also

- [[WORKFLOW]] — live configuration example
- [[docs/crates/symphony-config|symphony-config]] — implementation details
- [[SPEC]] S5-6 — canonical specification
