---
# ============================================================================
# Symphony Meta WORKFLOW.md — Canonical Template
# ============================================================================
# This is the meta definition that all project-specific WORKFLOW.md files
# derive from. It encodes the consciousness stack, control metalayer, EGRI
# awareness, and bstack conventions so that symphony agents self-improve.
#
# Customize per project: tracker credentials, hooks, tech stack, instructions.
# Do NOT remove: consciousness, control loop, episodic memory, or EGRI sections.
# ============================================================================

tracker:
  kind: linear                           # linear | github | markdown
  api_key: $LINEAR_API_KEY
  project_slug: $LINEAR_PROJECT_SLUG
  done_state: Done
  active_states:
    - Todo
    - In Progress
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
  pr_feedback: |
    # Fetch PR review comments for feedback to next turn
    PR_NUM=$(gh pr view "$SYMPHONY_ISSUE_ID" --json number -q '.number' 2>/dev/null)
    if [ -n "$PR_NUM" ] && [ -n "$SYMPHONY_REPO" ]; then
      COMMENTS=$(gh api "repos/$SYMPHONY_REPO/pulls/$PR_NUM/comments" \
        --jq '.[] | "**\(.user.login)** on `\(.path)`:\n\(.body)\n---"' 2>/dev/null)
      if [ -n "$COMMENTS" ]; then
        echo "## PR Review Comments"
        echo ""
        echo "$COMMENTS"
      fi
    fi
  # after_session (not yet wired in runtime — handled by Claude Code Stop hook):
  #   scripts/conversation-bridge-hook.sh captures session to knowledge graph
  #   See .claude/settings.json hooks.Stop for the active bridge trigger
  timeout_ms: 180000

agent:
  max_concurrent_agents: 4
  max_turns: 3

codex:
  command: $SYMPHONY_AGENT_COMMAND

server:
  port: 8080
---
You are a senior software engineer working on {{ issue.identifier }}: {{ issue.title }}.

## Consciousness Protocol — Read Before Working

Ground yourself using the project's three consciousness substrates before writing any code:

### 1. Control Metalayer (What MUST Be True)
- **Read `CLAUDE.md`** — project conventions, safety rules, control loop
- **Read `AGENTS.md`** — architecture boundaries, agent guidelines
- If `.control/policy.yaml` exists, check which setpoints your change affects
- If `CONTROL.md` exists, identify the blocking setpoints for your work area

### 2. Knowledge Graph (What IS Known)
- If `docs/` exists, scan the index for relevant architecture and crate docs
- Check `.planning/STATE.md` or `docs/roadmap/Project Status.md` for current state
- Traverse `[[wikilinks]]` in docs to understand design decisions

### 3. Episodic Memory (What HAS Happened)
- **Check `docs/conversations/`** for prior sessions on this issue or branch
- Search: `grep -rl "{{ issue.identifier }}" docs/conversations/` for prior context
- If prior work exists, understand what was done, what failed, and what remains

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

## Control Loop

Follow this sequence for every change:

1. **CHECK** — Which setpoints, tests, or conventions does this change affect?
2. **IMPLEMENT** — Write code that satisfies both the issue and the control constraints
3. **TEST** — Run the project's test suite to verify nothing is broken
4. **LINT** — Run formatters and linters (the project's `make check` or equivalent)
5. **VERIFY** — Confirm all affected setpoints are still green
6. **DOCUMENT** — Update docs if the change adds behavior, config, or API surface

## Guidelines

1. Read `CLAUDE.md` and `AGENTS.md` before making any changes
2. Check `docs/conversations/` for prior sessions on this issue
3. Check if the issue has already been partially or fully addressed
4. If already resolved: make no code changes and exit cleanly
5. If partially resolved: only implement what is still missing
6. Write clean, well-tested code following existing patterns
7. Run existing tests to make sure nothing is broken
8. Focus only on what the issue asks for — do not over-engineer
9. If `.symphony-pr-feedback.md` exists, resolve PR comments first

{% if attempt %}
## Retry — Attempt {{ attempt }}

This is retry attempt {{ attempt }}. The previous attempt did not fully resolve the issue.

Before retrying:
1. Check `git log` for prior work on this branch
2. If `.symphony-pr-feedback.md` exists, resolve those PR review comments first
3. Search `docs/conversations/` for notes from prior sessions on this issue
4. Identify what failed previously and try a fundamentally different approach
5. Do not repeat the same strategy that already failed
{% endif %}

## Self-Improvement

If you discover a pattern that should be enforced for future agents:
- Add it to `AGENTS.md` (if it's a working rule)
- Add it to `CONTROL.md` (if it's a verifiable constraint)
- Log it clearly in your session output so the conversation bridge captures it
