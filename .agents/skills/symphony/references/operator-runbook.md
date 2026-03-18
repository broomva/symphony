# Symphony Operator Runbook

Executable diagnostic recipes for operating Symphony runtimes.

## Pre-flight Check

```bash
symphony doctor                # checks WORKFLOW.md, env vars, binaries, daemon
```

## Check Daemon Health

```bash
curl -s http://localhost:8080/healthz    # 200 = alive
curl -s http://localhost:8080/readyz     # 200 = ready, 503 = initializing
symphony status                          # full daemon state
symphony status --format json | jq .     # machine-readable
```

## Debug Stuck Issue

```bash
symphony issue STI-123                      # check state, turn count, tokens
symphony logs --id STI-123                  # filter logs for this issue
symphony logs --id STI-123 --level error    # errors only
symphony logs --since 30m --id STI-123      # last 30 minutes
```

## Analyze Token Usage

```bash
symphony status                             # total tokens in status output
curl -s http://localhost:8080/api/v1/metrics | jq .totals
curl -s http://localhost:8080/metrics | grep symphony_tokens  # Prometheus
```

## Inspect Workspaces

```bash
symphony workspaces                         # list all
symphony workspace STI-123                  # detail for one
symphony workspace STI-123 --clean          # remove workspace directory
ls ~/symphony-workspaces/project/           # manual inspection
```

## Troubleshoot Hooks

```bash
# Test hooks in isolation:
export SYMPHONY_ISSUE_ID="STI-123"
export SYMPHONY_ISSUE_TITLE="Test issue"
bash -x -c 'git add -A && git commit -m "test"'    # test after_run
bash -x -c 'gh pr create --title "test" --body ""'  # test PR creation
```

## Force Immediate Poll

```bash
symphony refresh                            # trigger poll now
curl -X POST http://localhost:8080/api/v1/refresh  # direct API
```

## Stop Runaway Issue

```bash
# Move issue to terminal state in tracker (stops retries):
# Linear: set to "Done" or "Cancelled"
# GitHub: close the issue

# Or shut down the daemon entirely:
symphony stop
```

## View Prometheus Metrics

```bash
curl -s http://localhost:8080/metrics | grep -v '^#'  # values only
# Key metrics:
#   symphony_sessions_running      — active agent count
#   symphony_sessions_retrying     — retry queue depth
#   symphony_tokens_total          — cumulative token spend
#   symphony_issues_completed      — lifetime completions
```

## Arcan Runtime Diagnostics

```bash
curl -s http://localhost:3000/health          # Arcan daemon health
symphony status                               # shows arcan sessions
# If arcan is unreachable, check base_url in WORKFLOW.md runtime section
```
