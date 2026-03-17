#!/usr/bin/env bash
# Entropy check: reports code hygiene metrics for Symphony.
# Informational — always exits 0. Use to spot drift.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "=== Entropy Report ==="
echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo ""

# 1. #[allow(...)] count in Rust source
echo "--- Suppressed Warnings ---"
ALLOW_COUNT=$(grep -rn '#\[allow(' "$ROOT/crates" "$ROOT/src" 2>/dev/null | grep -v '/target/' | wc -l | tr -d ' ')
echo "  #[allow(...)] annotations: $ALLOW_COUNT"

# 2. TODO/FIXME/HACK count
echo ""
echo "--- Code Debt Markers ---"
for MARKER in TODO FIXME HACK; do
  COUNT=$(grep -rn "$MARKER" "$ROOT/crates" "$ROOT/src" 2>/dev/null | grep -v '/target/' | wc -l | tr -d ' ')
  echo "  $MARKER: $COUNT"
done

# 3. Doc staleness — files not modified in 60+ days
echo ""
echo "--- Stale Documentation (60+ days) ---"
STALE=0
while IFS= read -r -d '' mdfile; do
  MOD_TS=$(stat -f %m "$mdfile" 2>/dev/null || stat -c %Y "$mdfile" 2>/dev/null || echo "0")
  NOW_TS=$(date "+%s")
  DAYS_OLD=$(( (NOW_TS - MOD_TS) / 86400 ))
  if [ "$DAYS_OLD" -ge 60 ]; then
    echo "  ${mdfile#$ROOT/} (${DAYS_OLD}d)"
    STALE=$((STALE + 1))
  fi
done < <(find "$ROOT/docs" -name '*.md' -print0 2>/dev/null)
[ "$STALE" -eq 0 ] && echo "  (none)"

# 4. Test count vs STATE.md recorded count
echo ""
echo "--- Test Count ---"
LIVE_TESTS=$(cd "$ROOT" && cargo test --workspace 2>&1 | grep "test result:" | awk '{sum += $4} END {print sum}')
STATE_TESTS=$(grep -oP 'Tests: \K[0-9]+' "$ROOT/.planning/STATE.md" 2>/dev/null || echo "unknown")
echo "  Live test count: $LIVE_TESTS"
echo "  STATE.md recorded: $STATE_TESTS"
if [ "$LIVE_TESTS" != "$STATE_TESTS" ] && [ "$STATE_TESTS" != "unknown" ]; then
  echo "  ⚠ Mismatch — STATE.md may need updating"
else
  echo "  ✓ Consistent"
fi

echo ""
echo "=== ENTROPY REPORT COMPLETE ==="
