# WORKFLOW.md Configuration Reference

## Structure

WORKFLOW.md has YAML frontmatter (config) + Liquid template body (agent prompt):

```markdown
---
# YAML config
tracker:
  kind: linear
  ...
---
Agent prompt with {{ issue.identifier }} template variables
```

## Tracker Section

### Linear
```yaml
tracker:
  kind: linear
  api_key: $LINEAR_API_KEY          # env var expansion with $
  endpoint: https://api.linear.app/graphql  # default
  project_slug: 71c211385593        # from Linear project URL
  active_states: [Todo, In Progress]
  terminal_states: [Done, Canceled, Duplicate]
  done_state: Done                  # optional: auto-transition on success
```

### GitHub Issues
```yaml
tracker:
  kind: github
  api_key: $GITHUB_TOKEN
  project_slug: owner/repo          # owner/repo format
  active_states: [open]             # labels can match too
  terminal_states: [closed]
  done_state: closed                # optional: auto-close on success
```

GitHub state mapping: if an issue has a label matching an active_states entry, that label is used as the state. Otherwise GitHub's native open/closed.

## Polling Section
```yaml
polling:
  interval_ms: 30000    # poll every 30s (default)
```

## Workspace Section
```yaml
workspace:
  root: ~/symphony-workspaces/project   # ~ and $VAR expanded
```

## Hooks Section
```yaml
hooks:
  after_create: |       # runs once on workspace creation (fatal on failure)
    gh repo clone org/repo . -- --depth 50
    git checkout -b "symphony/$SYMPHONY_ISSUE_ID"
  before_run: |         # runs before each turn (fatal on failure)
    git fetch origin main && git rebase origin/main || git rebase --abort
  after_run: |          # runs after each turn (failure ignored)
    git add -A && git commit -m "$SYMPHONY_ISSUE_ID: changes" || true
    git push -u origin "symphony/$SYMPHONY_ISSUE_ID" --force-with-lease || true
  before_remove: |      # runs before workspace cleanup (failure ignored)
    echo "cleaning up"
  pr_feedback: |        # captures stdout as PR review feedback for next turn
    PR_NUM=$(gh pr view "symphony/$SYMPHONY_ISSUE_ID" --json number -q '.number' 2>/dev/null)
    if [ -n "$PR_NUM" ]; then
      gh api "repos/org/repo/pulls/$PR_NUM/comments" \
        --jq '.[] | "**\(.user.login)**: \(.body)"' 2>/dev/null
    fi
  timeout_ms: 180000    # 3 min hook timeout (default: 60s)
```

Hook env vars: `$SYMPHONY_ISSUE_ID`, `$SYMPHONY_ISSUE_TITLE`

## Agent Section
```yaml
agent:
  max_concurrent_agents: 3   # parallel workers (default: 10)
  max_turns: 5               # turns per issue (default: 20)
  max_retry_backoff_ms: 300000  # max retry delay (default: 5min)
  max_concurrent_agents_by_state:
    todo: 1                  # per-state concurrency limits
    in progress: 2
```

## Codex Section
```yaml
codex:
  command: claude --dangerously-skip-permissions
  turn_timeout_ms: 3600000     # 1hr per turn
  read_timeout_ms: 5000        # handshake timeout
  stall_timeout_ms: 300000     # inactivity kill timeout
```

## Server Section
```yaml
server:
  port: 8080    # HTTP dashboard + API
```

## Template Variables

Available in the Liquid prompt body:

| Variable | Type | Description |
|----------|------|-------------|
| `issue.identifier` | string | e.g. "STI-123" |
| `issue.title` | string | Issue title |
| `issue.description` | string? | Issue body (may be null) |
| `issue.state` | string | Current state |
| `issue.priority` | int? | Priority (lower = higher) |
| `issue.labels` | string[] | Lowercase labels |
| `issue.url` | string? | Issue URL |
| `issue.blocked_by` | object[] | Blocker references |
| `attempt` | int? | Retry attempt (null on first run) |
