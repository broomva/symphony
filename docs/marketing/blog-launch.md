---
tags:
  - symphony
  - marketing
  - blog
type: marketing
status: draft
area: launch
created: 2026-03-17
---

# From Issue Tracker to Autonomous Agent: How Symphony Turns Your Backlog Into Working Code

## The Problem

Every engineering team has the same bottleneck: there are more issues in the backlog than humans to work on them. AI coding agents like Claude Code, Codex, and Cursor have gotten remarkably capable -- but using them is still a manual process. You open a terminal, paste an issue description, wait, review, iterate. One issue at a time.

What if your issue tracker was the control interface for an army of coding agents?

## Introducing Symphony

Symphony is an open-source Rust orchestration engine that polls your issue tracker (Linear or GitHub), creates isolated workspaces for each issue, runs coding agents automatically, and manages the full lifecycle: retries, concurrency, PR creation, and review feedback.

```
cargo install symphony-cli
symphony init
symphony start
```

That's it. Symphony watches your backlog and dispatches agents autonomously.

## How It Works

### The WORKFLOW.md Contract

Everything is configured in a single `WORKFLOW.md` file that lives in your repo:

```yaml
tracker:
  kind: linear                        # or github
  api_key: $LINEAR_API_KEY
  project_slug: your-project-slug
  active_states: [Todo, In Progress]
  terminal_states: [Done, Canceled]
  done_state: Done                    # auto-transition on success

hooks:
  after_create: |
    gh repo clone your-org/your-repo . -- --depth 50
    git checkout -b "symphony/$SYMPHONY_ISSUE_ID"
  after_run: |
    git add -A && git commit -m "$SYMPHONY_ISSUE_ID: $SYMPHONY_ISSUE_TITLE" || true
    git push -u origin "symphony/$SYMPHONY_ISSUE_ID" --force-with-lease || true
  pr_feedback: |
    # Fetch PR review comments for the next agent turn
    gh api "repos/your-org/your-repo/pulls/$PR_NUM/comments" \
      --jq '.[] | "**\(.user.login)**: \(.body)"'

agent:
  max_concurrent_agents: 3
  max_turns: 5

codex:
  command: claude --dangerously-skip-permissions
```

The prompt template below the YAML front matter uses Liquid syntax with access to the full issue object -- title, description, labels, blockers, priority, and retry attempt count.

### The Dispatch Loop

Every tick, Symphony:

1. **Polls** the tracker for issues in active states
2. **Sorts** by priority (lower first), then creation date
3. **Filters** by eligibility: concurrency limits, blocker dependencies, claimed status
4. **Dispatches** workers: creates workspace, runs clone hooks, renders the prompt, launches the agent
5. **Monitors**: multi-turn support, stall detection, timeout enforcement
6. **Handles exit**: commits, pushes, creates PRs, captures review feedback, schedules retries

If the agent exits normally and `done_state` is configured, Symphony automatically transitions the issue to Done in the tracker.

## What Makes This Different: The Control Metalayer

Most AI agent setups are "fire and forget" -- you prompt an agent and hope for the best. Symphony introduces a **control metalayer** that makes agent work verifiable and repeatable.

### Setpoints, Not Vibes

The metalayer is defined in a `CONTROL.md` file with explicit **setpoints** -- assertions about what must be true:

```
| S64 | GitHub issues normalized to Symphony Issue type | Unit test: normalize_github_issue |
| S69 | done_state transition called on normal worker exit | Code: scheduler calls set_issue_state |
| S71 | symphony init generates valid WORKFLOW.md | Integration test: init_then_validate_roundtrip |
```

Before writing any code, agents check which setpoints their change affects. After writing code, sensors (automated tests) verify the setpoints hold. If a setpoint must be temporarily relaxed, it's logged in a deviation log with justification.

This creates a **feedback loop**:

```
CHECK setpoints → IMPLEMENT → MEASURE (make smoke) → VERIFY → DOCUMENT → FEEDBACK
```

The metalayer ensures that every agent session -- whether run by a human, Claude Code, or Symphony itself -- produces verifiably correct output. It's the difference between "the agent wrote some code" and "the agent wrote code that satisfies 76 explicit quality gates."

### Knowledge Context Graph

Symphony repos are also **Obsidian vaults**. Every documentation file uses `[[wikilinks]]` to form a knowledge graph that agents can traverse:

```
CLAUDE.md → CONTROL.md → docs/operations/Control Harness.md
         → AGENTS.md → docs/architecture/Crate Map.md
         → PLANS.md → docs/roadmap/Project Status.md
```

When an agent needs to understand the architecture, it follows links from the entry point (`docs/Symphony Index.md`) to find crate maps, configuration references, and per-module documentation. Context compounds across sessions because the graph persists.

This is fundamentally different from stuffing a giant system prompt with project context. The knowledge graph is **navigable** -- agents find what they need by following links, not by reading everything upfront.

## The PR Review Loop

One of Symphony's most powerful features is the **PR review loop**. After the agent pushes code and creates a PR:

1. The `pr_feedback` hook runs, fetching review comments from GitHub/Linear
2. Comments are written to `.symphony-pr-feedback.md` in the workspace
3. On the next retry turn, the agent sees these comments and resolves them first
4. This continues until the PR is clean or max turns are exhausted

This means Symphony doesn't just write code -- it handles code review too. Other agents (or human reviewers) can leave comments on the PR, and Symphony will pick them up and address them.

## Real-World Dogfood: Stimulus

We built Symphony to orchestrate agents on our own product, [Stimulus](https://getstimulus.ai) -- an AI-native supplier relationship management platform.

The first full dogfood run:
- Symphony polled our Linear project and picked up **STI-644: Live Support Chat Not Available**
- It cloned the Stimulus monorepo (Next.js 14 + FastAPI + PostgreSQL)
- Claude Code implemented a Crisp live chat integration with a fallback support dialog
- Created 5 new files, modified 2 existing files, wrote 21 tests
- Pushed to a branch, created [PR #1055](https://github.com/GetStimulus/stimulus/pull/1055)
- Marked the Linear issue as Done

Total time: 11 minutes. Zero human intervention.

## Architecture

Symphony is built in Rust with a layered crate architecture matching the [spec](https://github.com/openai/symphony):

| Crate | Responsibility |
|-------|---------------|
| `symphony-core` | Domain model: Issue, State, Session, Workspace |
| `symphony-config` | WORKFLOW.md parser, Liquid templates, file watcher |
| `symphony-tracker` | Linear GraphQL + GitHub REST API clients |
| `symphony-workspace` | Per-issue directory lifecycle, hooks, path safety |
| `symphony-agent` | Agent subprocess management, JSON-RPC + simple modes |
| `symphony-orchestrator` | Poll loop, dispatch, reconciliation, retry, drain |
| `symphony-observability` | HTTP server, dashboard, Prometheus metrics, auth |

222 tests. 76 setpoints. Zero warnings. Apache 2.0 licensed.

## Getting Started

### Install

```bash
# From source (recommended)
cargo install symphony-cli

# Or via curl
curl -fsSL https://raw.githubusercontent.com/broomva/symphony/master/install.sh | sh

# Or Docker
docker pull ghcr.io/broomva/symphony:latest
```

### Initialize

```bash
cd your-project
symphony init                          # Linear template
symphony init --tracker github          # GitHub template
```

### Configure

Edit the generated `WORKFLOW.md`:
- Set your tracker API key (`$LINEAR_API_KEY` or `$GITHUB_TOKEN`)
- Set the project slug
- Update the repo clone command in hooks
- Customize the agent prompt

### Run

```bash
# Validate first
symphony validate WORKFLOW.md

# Start the daemon
symphony start WORKFLOW.md

# Or run a single issue
symphony run STI-123 --workflow-path WORKFLOW.md
```

### Monitor

```bash
symphony status                     # daemon state
symphony issues                     # running + retrying
symphony issue STI-123              # single issue detail
curl localhost:8080/metrics          # Prometheus metrics
open http://localhost:8080           # web dashboard
```

## What's Next

- **Symphony Cloud**: A managed service (next-forge monorepo) with dashboard, control plane, and multi-tenant orchestration
- **More trackers**: Jira, GitLab, Asana (the `TrackerClient` trait makes this straightforward)
- **Agent marketplace**: Different agent configs for different types of work (bug fixes vs. features vs. refactoring)

Symphony is open source and we'd love contributions. The `EXTENDING.md` guide covers how to add trackers and agent runners.

**GitHub**: [github.com/broomva/symphony](https://github.com/broomva/symphony)
**Install**: `cargo install symphony-cli`
**License**: Apache 2.0
