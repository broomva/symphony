---
# Symphony WORKFLOW.md — GitHub Issues + Claude Code
# NOTE: GitHub tracker is planned for a future release.
# This is a placeholder showing the intended configuration shape.

tracker:
  kind: github                      # Not yet implemented — see PLANS.md Phase 8.4
  api_key: $GITHUB_TOKEN
  project_slug: your-org/your-repo
  active_states:
    - open
  terminal_states:
    - closed

polling:
  interval_ms: 60000

workspace:
  root: ~/symphony-workspaces/github-project

hooks:
  after_create: |
    gh repo clone $SYMPHONY_PROJECT_SLUG . -- --depth 50
  before_run: |
    git fetch origin main && git rebase origin/main || git rebase --abort
  after_run: |
    git add -A
    git commit -m "fix: $SYMPHONY_ISSUE_ID" || true
    gh pr create --fill || gh pr edit --body "Updated by Symphony" || true
  timeout_ms: 120000

agent:
  max_concurrent_agents: 3
  max_turns: 3

codex:
  command: "claude --dangerously-skip-permissions"

server:
  port: 8082
---

You are working on GitHub issue {{ issue.identifier }}: {{ issue.title }}.

{% if issue.description %}
## Details
{{ issue.description }}
{% endif %}

Implement the fix or feature. Write tests. Follow the project's conventions.

{% if attempt %}
Retry attempt {{ attempt }}.
{% endif %}
