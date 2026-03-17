---
# Symphony WORKFLOW.md — GitHub Issues + Claude Code
# Polls a GitHub repo for open issues and runs Claude Code on each.

tracker:
  kind: github
  api_key: $GITHUB_TOKEN
  project_slug: your-org/your-repo   # owner/repo format
  active_states:
    - open
    - in progress                     # matches issues with "in progress" label
  terminal_states:
    - closed

polling:
  interval_ms: 60000

workspace:
  root: ~/symphony-workspaces/github-project

hooks:
  after_create: |
    gh repo clone $SYMPHONY_PROJECT_SLUG . -- --depth 50
    git checkout -b "symphony/$SYMPHONY_ISSUE_ID"
  before_run: |
    git add -A && git stash || true
    git fetch origin main
    git rebase origin/main || git rebase --abort
    git stash pop || true
  after_run: |
    git add -A
    git commit -m "$SYMPHONY_ISSUE_ID: $SYMPHONY_ISSUE_TITLE" || true
    git push -u origin "symphony/$SYMPHONY_ISSUE_ID" --force-with-lease || true
    gh pr view "symphony/$SYMPHONY_ISSUE_ID" --json state >/dev/null 2>&1 || \
      gh pr create --base main --head "symphony/$SYMPHONY_ISSUE_ID" \
        --title "$SYMPHONY_ISSUE_ID: $SYMPHONY_ISSUE_TITLE" \
        --body "Automated by Symphony" || true
  pr_feedback: |
    PR_NUM=$(gh pr view "symphony/$SYMPHONY_ISSUE_ID" --json number -q '.number' 2>/dev/null)
    if [ -n "$PR_NUM" ]; then
      COMMENTS=$(gh api "repos/${SYMPHONY_PROJECT_SLUG}/pulls/$PR_NUM/comments" \
        --jq '.[] | "**\(.user.login)** on `\(.path)`:\n\(.body)\n---"' 2>/dev/null)
      if [ -n "$COMMENTS" ]; then
        echo "## PR Review Comments"
        echo ""
        echo "$COMMENTS"
      fi
    fi
  timeout_ms: 120000

agent:
  max_concurrent_agents: 3
  max_turns: 5

codex:
  command: "claude --dangerously-skip-permissions"

server:
  port: 8082
---

You are working on GitHub issue {{ issue.identifier }}: {{ issue.title }}.

## Control Metalayer
1. Read CLAUDE.md and AGENTS.md for project conventions
2. Check `.symphony-pr-feedback.md` — resolve PR comments first if present
3. Follow: implement → test → lint → verify

{% if issue.description %}
## Details
{{ issue.description }}
{% endif %}

{% if issue.labels %}
## Labels
{{ issue.labels | join: ", " }}
{% endif %}

Implement the fix or feature. Write tests. Follow the project's conventions.

{% if attempt %}
## Retry Attempt {{ attempt }}
Check git log for prior work. If `.symphony-pr-feedback.md` exists, resolve those comments first.
{% endif %}
