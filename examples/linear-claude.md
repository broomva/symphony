---
# Symphony WORKFLOW.md — Linear + Claude Code
# Polls a Linear project for Todo issues and runs Claude Code on each.

tracker:
  kind: linear
  api_key: $LINEAR_API_KEY          # Set in environment or .env file
  project_slug: your-project-slug   # Find in Linear project settings
  active_states:
    - Todo
    - In Progress
  terminal_states:
    - Done
    - Canceled
    - Duplicate

polling:
  interval_ms: 30000                # Poll every 30 seconds

workspace:
  root: ~/symphony-workspaces/my-project

hooks:
  after_create: |
    # Clone the repo into the workspace on first creation
    gh repo clone your-org/your-repo . -- --depth 50
    git checkout -b "symphony/$SYMPHONY_ISSUE_ID"
  before_run: |
    # Rebase on main before each attempt
    git add -A && git stash || true
    git fetch origin main
    git rebase origin/main || git rebase --abort
    git stash pop || true
  after_run: |
    # Commit and push changes after each attempt
    git add -A
    git commit -m "$SYMPHONY_ISSUE_ID: automated changes" || true
    git push -u origin "symphony/$SYMPHONY_ISSUE_ID" --force-with-lease || true
  timeout_ms: 180000                # 3 minute hook timeout

agent:
  max_concurrent_agents: 2          # Run up to 2 agents simultaneously
  max_turns: 3                      # Max turns per issue before retry

codex:
  command: "claude --dangerously-skip-permissions"

server:
  port: 8080                        # Dashboard at http://localhost:8080
---

You are a senior software engineer working on {{ issue.identifier }}: {{ issue.title }}.

{% if issue.description %}
## Description
{{ issue.description }}
{% endif %}

{% if issue.labels %}
## Labels
{{ issue.labels | join: ", " }}
{% endif %}

## Instructions
- Read the codebase to understand the project structure
- Implement the requested changes
- Write tests for your changes
- Ensure all existing tests pass

{% if attempt %}
## Retry Attempt {{ attempt }}
This is a retry. Review what happened in the previous attempt and try a different approach.
{% endif %}
