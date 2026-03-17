#!/usr/bin/env bash
# EGRI Single Iteration — constraint-check → evaluate → log to ledger.
# Usage: bash evals/symphony-prompts/run_eval.sh [iteration_number]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
EVAL_DIR="$ROOT/evals/symphony-prompts"
LEDGER="$EVAL_DIR/ledger.jsonl"
ITERATION="${1:-1}"
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)

echo "=== EGRI Iteration $ITERATION ==="
echo "Timestamp: $TIMESTAMP"
echo ""

# Step 1: Constraint check
echo "--- Constraint Check ---"
CONSTRAINT_PASS="true"
if bash "$EVAL_DIR/constraint-check.sh"; then
  echo "Constraints: PASS"
else
  echo "Constraints: FAIL"
  CONSTRAINT_PASS="false"
fi
echo ""

# Step 2: Evaluate
echo "--- Evaluation ---"
EVAL_RESULT=$(bash "$EVAL_DIR/evaluator.sh" 2>&1) || true
SCORE=$(echo "$EVAL_RESULT" | jq -r '.score // "null"' 2>/dev/null || echo "null")
echo "Score: $SCORE"
echo "Detail: $EVAL_RESULT"
echo ""

# Step 3: Log to ledger
MUTATION_SUMMARY="${MUTATION_SUMMARY:-no mutation (baseline evaluation)}"
PROMOTED="false"

jq -nc \
  --argjson iteration "$ITERATION" \
  --arg timestamp "$TIMESTAMP" \
  --arg score "$SCORE" \
  --arg constraint_pass "$CONSTRAINT_PASS" \
  --arg mutation_summary "$MUTATION_SUMMARY" \
  --arg promoted "$PROMOTED" \
  '{
    iteration: $iteration,
    timestamp: $timestamp,
    score: (if $score == "null" then null else ($score | tonumber) end),
    constraint_pass: ($constraint_pass == "true"),
    mutation_summary: $mutation_summary,
    promoted: ($promoted == "true")
  }' >> "$LEDGER"

echo "--- Ledger Entry ---"
tail -1 "$LEDGER"
echo ""
echo "=== EGRI Iteration $ITERATION Complete ==="
