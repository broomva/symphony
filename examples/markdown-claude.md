---
# Symphony WORKFLOW.md — Markdown Files + Claude Code
# No external API or credentials required.
# Issues are .md files in the project_slug directory.

tracker:
  kind: markdown
  endpoint: ""                           # Optional: Lago HTTP endpoint for journaling
                                         # e.g. http://localhost:8080
  project_slug: ./tasks/                 # Directory containing issue .md files
  active_states:
    - Todo
    - In Progress
  terminal_states:
    - Done
    - Canceled
  done_state: Done                       # Auto-transition on success

polling:
  interval_ms: 10000                     # Local FS is fast, poll frequently

workspace:
  root: ~/symphony-workspaces/my-project

hooks:
  after_create: |
    cp -r /path/to/your/project/. . || true
    git init . 2>/dev/null || true
    git checkout -b "symphony/$SYMPHONY_ISSUE_ID" || true
  before_run: |
    git add -A && git stash || true
    git stash pop || true
  after_run: |
    git add -A
    git commit -m "$SYMPHONY_ISSUE_ID: $SYMPHONY_ISSUE_TITLE" || true
  timeout_ms: 120000

agent:
  max_concurrent_agents: 2
  max_turns: 3

codex:
  command: claude --dangerously-skip-permissions

server:
  port: 8084
---

You are a senior engineer working on {{ issue.identifier }}: {{ issue.title }}.

## Control Metalayer
1. Read CLAUDE.md and AGENTS.md for project conventions
2. Follow: implement → test → lint → verify

{% if issue.description %}
## Issue Description
{{ issue.description }}
{% endif %}

{% if issue.labels %}
## Labels
{{ issue.labels | join: ", " }}
{% endif %}

## Instructions
1. Read the project structure and understand the codebase
2. Implement the requested changes following existing patterns
3. Write or update tests for your changes
4. Ensure the code compiles and lints cleanly
5. Keep changes focused — only modify what the ticket requires

{% if attempt %}
## Retry Attempt {{ attempt }}
Check git log for prior work and avoid repeating failed approaches.
{% endif %}
