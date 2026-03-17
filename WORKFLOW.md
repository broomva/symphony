---
tracker:
  kind: linear
  api_key: $LINEAR_API_KEY
  project_slug: $LINEAR_PROJECT_SLUG
  done_state: Done
  active_states:
    - Todo
  terminal_states:
    - Done
    - Canceled
    - Duplicate
polling:
  interval_ms: 30000
workspace:
  root: $SYMPHONY_WORKSPACE_ROOT
hooks:
  after_create: |
    gh repo clone $SYMPHONY_REPO . -- --depth 50
    git checkout -b "$SYMPHONY_ISSUE_ID"
  before_run: |
    git add -A
    git stash || true
    git fetch origin $SYMPHONY_BASE_BRANCH
    git rebase origin/$SYMPHONY_BASE_BRANCH || git rebase --abort
    git stash pop || true
  after_run: |
    git add -A
    git diff --cached --quiet && NO_CHANGES=true || NO_CHANGES=false
    if [ "$NO_CHANGES" = "false" ]; then
      COMMIT_TITLE="${SYMPHONY_ISSUE_ID}: ${SYMPHONY_ISSUE_TITLE:-automated changes}"
      git commit -m "$COMMIT_TITLE"
      git push -u origin "$SYMPHONY_ISSUE_ID" --force-with-lease || true
      if ! gh pr view "$SYMPHONY_ISSUE_ID" --json state >/dev/null 2>&1; then
        PR_BODY="Automated changes by Symphony agent for $SYMPHONY_ISSUE_ID - $SYMPHONY_ISSUE_TITLE"
        gh pr create \
          --title "$COMMIT_TITLE" \
          --body "$PR_BODY" \
          --base "$SYMPHONY_BASE_BRANCH" \
          --head "$SYMPHONY_ISSUE_ID" || true
      fi
    fi
  timeout_ms: 180000
agent:
  max_concurrent_agents: 4
  max_turns: 3
codex:
  command: $SYMPHONY_AGENT_COMMAND
server:
  port: 8080
---
You are a senior software engineer working on the {{ issue.identifier }} project.

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

{% if issue.blocked_by.size > 0 %}
## Dependencies
{% for blocker in issue.blocked_by %}
- {{ blocker.identifier }}: {{ blocker.state }}
{% endfor %}
{% endif %}

## Guidelines
1. Read the relevant parts of the codebase before making changes
2. Check if the issue has already been partially or fully addressed
3. If already resolved: make no code changes and exit cleanly
4. If partially resolved: only implement what is still missing
5. Write clean, well-tested code following existing patterns
6. Run existing tests to make sure nothing is broken
7. Focus only on what the issue asks for — do not over-engineer

{% if attempt %}
## Retry
This is retry attempt {{ attempt }}. The previous attempt failed.
Review what went wrong and try a different approach.
{% endif %}
