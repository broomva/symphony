# Symphony Troubleshooting

## Common Issues

### "Not logged in · Please run /login"
**Cause**: Claude Code CLI not authenticated in the environment.
**Fix**: Set `ANTHROPIC_API_KEY` in the environment where Symphony runs.
```bash
export ANTHROPIC_API_KEY=sk-ant-...
```
For Docker/Railway: add as environment variable in the service config.

### Agent exits immediately (exit code 1)
**Cause**: The `codex.command` binary not found or not executable.
**Fix**: Verify the command works standalone:
```bash
claude --version  # must be installed and in PATH
```

### "tracker.api_key is required (after $VAR resolution)"
**Cause**: Environment variable not set.
**Fix**: Export the variable before running:
```bash
export LINEAR_API_KEY=lin_api_...   # for Linear
export GITHUB_TOKEN=ghp_...         # for GitHub
```

### "unsupported_tracker_kind"
**Cause**: `tracker.kind` is not "linear" or "github".
**Fix**: Check WORKFLOW.md frontmatter for typos.

### Hooks failing with permission errors
**Cause**: `gh` CLI not authenticated.
**Fix**: Run `gh auth login` in the environment. For Docker, mount `~/.config/gh/`.

### Issues stuck in retry (attempt 30+)
**Cause**: Agent keeps failing on the same issue.
**Fix**:
1. Check logs: `symphony logs --id STI-123`
2. Common causes: agent auth, repo clone failure, test failures
3. Reduce max_turns to avoid burning tokens
4. Move the issue to a terminal state in the tracker to stop retries

### PR not auto-created
**Cause**: `after_run` hook doesn't include PR creation.
**Fix**: Add to hooks section:
```yaml
after_run: |
  git add -A && git commit -m "$SYMPHONY_ISSUE_ID: changes" || true
  git push -u origin "symphony/$SYMPHONY_ISSUE_ID" --force-with-lease || true
  gh pr view "symphony/$SYMPHONY_ISSUE_ID" --json state >/dev/null 2>&1 || \
    gh pr create --base main --head "symphony/$SYMPHONY_ISSUE_ID" \
      --title "$SYMPHONY_ISSUE_ID: $SYMPHONY_ISSUE_TITLE" \
      --body "Automated by Symphony" || true
```

### Workspace path errors
**Cause**: `workspace.root` doesn't exist or uses unsupported path.
**Fix**: Create the directory first, use absolute paths or `~`:
```bash
mkdir -p ~/symphony-workspaces/project
```

## Monitoring

### HTTP Dashboard
```
http://localhost:8080          # HTML dashboard
http://localhost:8080/healthz  # liveness (always 200)
http://localhost:8080/readyz   # readiness (200 when initialized)
http://localhost:8080/metrics  # Prometheus text format
```

### API Endpoints (require SYMPHONY_API_TOKEN if set)
```
GET  /api/v1/state          # system summary JSON
GET  /api/v1/{identifier}   # single issue detail
GET  /api/v1/workspaces     # list workspaces
GET  /api/v1/metrics        # usage metrics JSON
POST /api/v1/refresh        # trigger immediate poll
POST /api/v1/shutdown       # graceful shutdown
```

### Prometheus Metrics
```
symphony_tokens_input_total
symphony_tokens_output_total
symphony_tokens_total
symphony_agent_seconds_total
symphony_sessions_running
symphony_sessions_retrying
symphony_issues_claimed
symphony_issues_completed
symphony_config_poll_interval_ms
symphony_config_max_concurrent_agents
```
