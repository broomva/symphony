#!/usr/bin/env bash
# EGRI Evaluator — queries Symphony API and computes resolution rate score.
# Returns JSON: { "score": 0.0-1.0, "detail": { ... } }
# Exits 0 on success, 1 on connection error (graceful).
set -euo pipefail

ENDPOINT="${SYMPHONY_EVAL_ENDPOINT:-http://localhost:8080/api/v1/state}"
TOKEN="${SYMPHONY_API_TOKEN:-}"

# Build auth header if token is set
AUTH_HEADER=""
if [ -n "$TOKEN" ]; then
  AUTH_HEADER="-H \"Authorization: Bearer $TOKEN\""
fi

# Query the state endpoint
RESPONSE=$(eval curl -s -f "$AUTH_HEADER" "$ENDPOINT" 2>/dev/null) || {
  echo '{"score": null, "error": "connection_failed", "detail": "Symphony not running or unreachable at '"$ENDPOINT"'"}'
  exit 1
}

# Extract counts using jq
if ! command -v jq >/dev/null 2>&1; then
  echo '{"score": null, "error": "jq_not_found", "detail": "jq is required for score computation"}'
  exit 1
fi

COMPLETED=$(echo "$RESPONSE" | jq -r '.sessions_completed // 0')
FAILED=$(echo "$RESPONSE" | jq -r '.sessions_failed // 0')
RETRYING=$(echo "$RESPONSE" | jq -r '.sessions_retrying // .retry_queue_size // 0')
RUNNING=$(echo "$RESPONSE" | jq -r '.sessions_running // .active_workers // 0')

TOTAL=$((COMPLETED + FAILED + RETRYING))

if [ "$TOTAL" -eq 0 ]; then
  SCORE="0.0"
else
  # Compute score as completed / total (using awk for float division)
  SCORE=$(awk "BEGIN {printf \"%.4f\", $COMPLETED / $TOTAL}")
fi

# Output structured result
jq -n \
  --arg score "$SCORE" \
  --argjson completed "$COMPLETED" \
  --argjson failed "$FAILED" \
  --argjson retrying "$RETRYING" \
  --argjson running "$RUNNING" \
  --argjson total "$TOTAL" \
  '{
    score: ($score | tonumber),
    detail: {
      completed: $completed,
      failed: $failed,
      retrying: $retrying,
      running: $running,
      total: $total
    }
  }'
