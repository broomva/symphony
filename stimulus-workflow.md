---
# Symphony WORKFLOW.md — Stimulus SRM (Vendor Interest Form Fixes)
# Polls Linear project for In Progress issues assigned to the team,
# clones GetStimulus/stimulus, and runs Claude Code on each.

tracker:
  kind: linear
  api_key: $LINEAR_API_KEY
  project_slug: $LINEAR_PROJECT_SLUG
  active_states:
    - In Progress
    - In Review
  terminal_states:
    - Done
    - Canceled
    - Duplicate

polling:
  interval_ms: 60000              # Poll every 60 seconds

workspace:
  root: $SYMPHONY_WORKSPACE_ROOT

hooks:
  after_create: |
    # Clone the Stimulus repo into the workspace on first creation
    gh repo clone $SYMPHONY_REPO . -- --depth 50
    git checkout $SYMPHONY_BASE_BRANCH
    git checkout -b "symphony/$SYMPHONY_ISSUE_ID"
  before_run: |
    # Rebase on base branch before each attempt
    git add -A && git stash || true
    git fetch origin $SYMPHONY_BASE_BRANCH
    git rebase origin/$SYMPHONY_BASE_BRANCH || git rebase --abort
    git stash pop || true
  after_run: |
    # Commit and push changes after each attempt
    git add -A
    git commit -m "$SYMPHONY_ISSUE_ID: $SYMPHONY_ISSUE_TITLE" || true
    git push -u origin "symphony/$SYMPHONY_ISSUE_ID" --force-with-lease || true
    # Create PR if it doesn't exist
    gh pr view "symphony/$SYMPHONY_ISSUE_ID" --json state >/dev/null 2>&1 || \
      gh pr create --base $SYMPHONY_BASE_BRANCH --head "symphony/$SYMPHONY_ISSUE_ID" \
        --title "$SYMPHONY_ISSUE_ID: $SYMPHONY_ISSUE_TITLE" \
        --body "Automated by Symphony. Issue: $SYMPHONY_ISSUE_ID" || true
  pr_feedback: |
    # Fetch PR review comments for feedback to next turn
    PR_NUM=$(gh pr view "symphony/$SYMPHONY_ISSUE_ID" --json number -q '.number' 2>/dev/null)
    if [ -n "$PR_NUM" ]; then
      COMMENTS=$(gh api "repos/$SYMPHONY_REPO/pulls/$PR_NUM/comments" \
        --jq '.[] | "**\(.user.login)** on `\(.path)`:\n\(.body)\n---"' 2>/dev/null)
      if [ -n "$COMMENTS" ]; then
        echo "## PR Review Comments"
        echo ""
        echo "$COMMENTS"
      fi
    fi
  timeout_ms: 300000              # 5 minute hook timeout

agent:
  max_concurrent_agents: 1        # Start with 1 agent for first dogfood
  max_turns: 5                    # Allow up to 5 turns per issue

codex:
  command: $SYMPHONY_AGENT_COMMAND

server:
  port: 8080
---

You are a senior full-stack engineer working on {{ issue.identifier }}: {{ issue.title }}.

You are working in the GetStimulus/stimulus monorepo — a Next.js 14 + FastAPI + PostgreSQL SRM (Supplier Relationship Management) platform.

## Control Metalayer

Before making any changes, ground yourself using the project's control framework:

1. **Read CLAUDE.md and AGENTS.md** — project conventions and safety rules
2. **Check for existing tests** — understand current coverage before modifying
3. **Follow the control loop**: implement → test → lint → verify → document
4. **Check `.symphony-pr-feedback.md`** — if this file exists, it contains PR review comments from previous turns that you MUST address before doing other work

## Tech Stack
- **Frontend**: Next.js 14, TypeScript, Clerk auth, RTK Query, Tailwind CSS
- **Backend**: FastAPI (Python), SQLAlchemy, multi-tenant PostgreSQL (Neon)
- **AI**: AI SDK v6 (ToolLoopAgent), MCP server
- **Infra**: Azure (AKS, ACR), Databricks

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
2. Check for any existing CLAUDE.md or AGENTS.md for project conventions
3. If `.symphony-pr-feedback.md` exists, resolve those PR comments first
4. Implement the requested changes following existing patterns
5. Write or update tests for your changes
6. Ensure the code compiles and lints cleanly
7. Keep changes focused — only modify what the ticket requires

{% if attempt %}
## Retry Attempt {{ attempt }}
This is retry attempt {{ attempt }}. Review what happened previously and try a different approach.
Check git log for prior work on this branch.
If `.symphony-pr-feedback.md` exists, prioritize resolving those review comments.
{% endif %}
