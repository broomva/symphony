---
tracker:
  kind: linear
  api_key: $LINEAR_API_KEY
  project_slug: 71c211385593
  active_states:
    - Todo
  terminal_states:
    - Done
    - Canceled
    - Duplicate
polling:
  interval_ms: 30000
workspace:
  root: ~/symphony-workspaces/stimulus
hooks:
  after_create: |
    gh repo clone GetStimulus/stimulus . -- --depth 50
    git checkout -b "$SYMPHONY_ISSUE_ID"
  before_run: |
    git fetch origin main
    git rebase origin/main || git rebase --abort
  after_run: |
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
You are a senior software engineer working on the Stimulus platform.

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
- Focus only on what the issue asks for — do not over-engineer

{% if attempt %}
## Retry
This is retry attempt {{ attempt }}. The previous attempt failed.
Review what went wrong and try a different approach.
{% endif %}
