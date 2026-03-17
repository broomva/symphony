#!/usr/bin/env bash
# Validates .control/policy.yaml setpoint IDs against CONTROL.md.
# Reports any IDs present in one but not the other.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FAIL=0

echo "=== Policy Validation ==="

# Extract setpoint IDs from policy.yaml
POLICY_IDS=$(grep '^\s*- id: S' "$ROOT/.control/policy.yaml" | sed 's/.*id: //' | sort -V)

# Extract setpoint IDs from CONTROL.md (table rows starting with | S<num>)
CONTROL_IDS=$(grep -oE '\| S[0-9]+' "$ROOT/CONTROL.md" | sed 's/| //' | sort -uV)

# Compare
echo ""
echo "--- Setpoints in policy.yaml but NOT in CONTROL.md ---"
EXTRA=0
for id in $POLICY_IDS; do
  if ! echo "$CONTROL_IDS" | grep -qx "$id"; then
    echo "  $id"
    EXTRA=1
    FAIL=1
  fi
done
[ "$EXTRA" -eq 0 ] && echo "  (none)"

echo ""
echo "--- Setpoints in CONTROL.md but NOT in policy.yaml ---"
MISSING=0
for id in $CONTROL_IDS; do
  if ! echo "$POLICY_IDS" | grep -qx "$id"; then
    echo "  $id"
    MISSING=1
    FAIL=1
  fi
done
[ "$MISSING" -eq 0 ] && echo "  (none)"

echo ""
POLICY_COUNT=$(echo "$POLICY_IDS" | wc -l | tr -d ' ')
CONTROL_COUNT=$(echo "$CONTROL_IDS" | wc -l | tr -d ' ')
echo "Summary: policy.yaml has $POLICY_COUNT setpoints, CONTROL.md has $CONTROL_COUNT setpoints"

if [ "$FAIL" -eq 0 ]; then
  echo ""
  echo "=== POLICY VALIDATION PASS ==="
else
  echo ""
  echo "=== POLICY VALIDATION FAIL ==="
  exit 1
fi
