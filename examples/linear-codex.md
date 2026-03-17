---
# Symphony WORKFLOW.md — Linear + OpenAI Codex
# Uses OpenAI's Codex app-server as the coding agent.

tracker:
  kind: linear
  api_key: $LINEAR_API_KEY
  project_slug: your-project-slug
  active_states:
    - Todo
  terminal_states:
    - Done
    - Canceled

polling:
  interval_ms: 60000                # Poll every 60 seconds

workspace:
  root: ~/symphony-workspaces/codex-project

hooks:
  after_create: |
    gh repo clone your-org/your-repo . -- --depth 50
  before_run: |
    git fetch origin main
    git checkout main
    git pull origin main
  timeout_ms: 120000

agent:
  max_concurrent_agents: 1
  max_turns: 5

codex:
  command: "codex-app-server"
  approval_policy: "auto-edit"
  turn_timeout_ms: 600000           # 10 minute turn timeout
  stall_timeout_ms: 300000          # 5 minute stall detection

server:
  port: 8081
---

You are working on issue {{ issue.identifier }}: {{ issue.title }}.

{% if issue.description %}
{{ issue.description }}
{% endif %}

Implement the requested changes. Write clean, tested code.

{% if attempt %}
Retry attempt {{ attempt }}. Try a different approach.
{% endif %}
