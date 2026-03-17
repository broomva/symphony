---
name: symphony
description: Expert guidance for Symphony — the Rust orchestration engine for coding agents. Use when users ask about installing Symphony (cargo install, curl|bash, docker), initializing projects (symphony init), configuring WORKFLOW.md for Linear or GitHub trackers, running in daemon or one-shot mode, setting up the control metalayer (CONTROL.md, setpoints), configuring lifecycle hooks (after_create, before_run, after_run, pr_feedback), monitoring with HTTP dashboard and Prometheus /metrics, extending with new trackers, or troubleshooting agent/hook failures. Triggers on "symphony", "WORKFLOW.md", "coding agent orchestration", "issue tracker automation", "control metalayer", or "agent dispatch".
---

# Symphony

Rust orchestration engine that polls issue trackers (Linear, GitHub), creates isolated workspaces, and runs coding agents automatically.

## Install

```bash
cargo install symphony-cli                # from source
curl -fsSL https://raw.githubusercontent.com/broomva/symphony/master/install.sh | sh  # binary
docker pull ghcr.io/broomva/symphony:latest  # container
```

## Quick Start

```bash
symphony init                          # scaffold WORKFLOW.md (Linear default)
symphony init --tracker github          # GitHub Issues template
# Edit WORKFLOW.md — fill in project details, API key, repo
symphony validate WORKFLOW.md           # verify config
symphony start WORKFLOW.md              # run daemon
```

## Commands

| Command | Purpose |
|---------|---------|
| `symphony init [--tracker github]` | Scaffold WORKFLOW.md |
| `symphony start [WORKFLOW.md]` | Start daemon (polls tracker) |
| `symphony run STI-123 --workflow-path WORKFLOW.md` | One-shot single issue |
| `symphony status` | Query daemon state |
| `symphony issues` | List running + retrying |
| `symphony issue STI-123` | Detail for one issue |
| `symphony refresh` | Trigger immediate poll |
| `symphony stop` | Graceful shutdown |
| `symphony validate WORKFLOW.md` | Validate config + template |
| `symphony config WORKFLOW.md` | Show resolved config |

Flags: `--port`, `--host`, `--token`, `--format json`, `--concurrency`, `--turns`, `--once`, `--tickets STI-1,STI-2`

## WORKFLOW.md

YAML frontmatter (config) + Liquid template body (agent prompt). For complete reference: [references/workflow-config.md](references/workflow-config.md).

Minimal example:
```yaml
---
tracker:
  kind: linear              # or github
  api_key: $LINEAR_API_KEY
  project_slug: your-slug
codex:
  command: claude --dangerously-skip-permissions
---
Fix {{ issue.identifier }}: {{ issue.title }}
{{ issue.description }}
```

## Agent Lifecycle

```
Poll tracker → fetch active issues → sort by priority → dispatch workers
    ├─ after_create hook (clone repo)
    ├─ before_run hook (rebase)
    ├─ render prompt + run agent (max_turns)
    ├─ after_run hook (commit, push, create PR)
    ├─ pr_feedback hook (capture review comments)
    ├─ done_state transition (auto-close issue)
    └─ retry (1s continuation / exponential backoff)
```

## Control Metalayer

The control metalayer (CONTROL.md) grounds all development:
```
CHECK setpoints → IMPLEMENT → MEASURE (make smoke) → VERIFY → DOCUMENT → FEEDBACK
```

Set up: create `CONTROL.md` with setpoints, add sensors in `Makefile`, reference in agent prompt.

## Extending

Implement `TrackerClient` trait (4 methods: `fetch_candidate_issues`, `fetch_issues_by_states`, `fetch_issue_states_by_ids`, `set_issue_state`). Register in `create_tracker()` factory.

## Key Environment Variables

| Variable | Purpose |
|----------|---------|
| `LINEAR_API_KEY` | Linear API auth |
| `GITHUB_TOKEN` | GitHub API auth |
| `ANTHROPIC_API_KEY` | Claude Code auth |
| `SYMPHONY_API_TOKEN` | Symphony HTTP API auth |
| `SYMPHONY_PORT` | HTTP server port |

## Troubleshooting

See [references/troubleshooting.md](references/troubleshooting.md) for auth failures, stuck retries, hook errors, missing PRs, and monitoring setup.
