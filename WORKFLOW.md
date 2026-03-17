---
tracker:
  kind: linear
  api_key: $LINEAR_API_KEY
  project_slug: a772f4e5ab68
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
    git add -A
    git stash || true
    git fetch origin main
    git rebase origin/main || git rebase --abort
    git stash pop || true
  after_run: |
    # Commit any changes
    git add -A
    git diff --cached --quiet && NO_CHANGES=true || NO_CHANGES=false
    if [ "$NO_CHANGES" = "false" ]; then
      git commit -m "$SYMPHONY_ISSUE_ID: automated changes"
      git push -u origin "$SYMPHONY_ISSUE_ID" --force-with-lease || true
      # Create PR if one doesn't exist
      if ! gh pr view "$SYMPHONY_ISSUE_ID" --json state >/dev/null 2>&1; then
        gh pr create \
          --title "$SYMPHONY_ISSUE_ID: automated changes" \
          --body "Automated changes by Symphony agent for $SYMPHONY_ISSUE_ID" \
          --head "$SYMPHONY_ISSUE_ID" || true
      fi
    fi
    # Move Linear issue to Done so Symphony stops re-dispatching
    # Parse team key and number from identifier (e.g. STI-339 → STI, 339)
    TEAM_KEY=$(echo "$SYMPHONY_ISSUE_ID" | cut -d'-' -f1)
    ISSUE_NUM=$(echo "$SYMPHONY_ISSUE_ID" | cut -d'-' -f2)
    ISSUE_UUID=$(curl -s -X POST https://api.linear.app/graphql \
      -H "Authorization: $LINEAR_API_KEY" \
      -H "Content-Type: application/json" \
      -d "{\"query\":\"{ issues(filter: { team: { key: { eq: \\\"$TEAM_KEY\\\" } }, number: { eq: $ISSUE_NUM } }) { nodes { id } } }\"}" \
      | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['issues']['nodes'][0]['id'])" 2>/dev/null) || true
    if [ -n "$ISSUE_UUID" ]; then
      curl -s -X POST https://api.linear.app/graphql \
        -H "Authorization: $LINEAR_API_KEY" \
        -H "Content-Type: application/json" \
        -d "{\"query\":\"mutation { issueUpdate(id: \\\"$ISSUE_UUID\\\", input: { stateId: \\\"6feb8707-bae8-48fe-87a8-bfd66016ca03\\\" }) { success } }\"}" || true
    fi
  timeout_ms: 180000
agent:
  max_concurrent_agents: 4
  max_turns: 3
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

## Important: Ticket Triage
This ticket is from an older backlog. Before implementing anything:
1. Read the relevant parts of the codebase to understand the CURRENT state
2. Check if the issue has already been partially or fully addressed
3. If the issue is already resolved: make no code changes and exit cleanly
4. If partially resolved: only implement what is still missing
5. If still relevant: proceed with full implementation

## Instructions
- Read the codebase thoroughly before making changes
- Write clean, well-tested code following existing patterns
- Run existing tests to make sure nothing is broken
- Focus only on what the issue asks for — do not over-engineer
- If you determine this issue is outdated or already fixed, that is a valid outcome — document your findings in a brief comment

{% if attempt %}
## Retry
This is retry attempt {{ attempt }}. The previous attempt failed.
Review what went wrong and try a different approach.
{% endif %}
