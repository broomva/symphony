#!/usr/bin/env bash
# Regenerates .control/state.json from live measurements.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
STATE_FILE="$ROOT/.control/state.json"

echo "=== Refreshing .control/state.json ==="

# Version from workspace Cargo.toml
VERSION=$(grep -A2 '^\[workspace\.package\]' "$ROOT/Cargo.toml" | grep '^version' | sed 's/.*"\(.*\)"/\1/')

# Test counts
TEST_OUTPUT=$(cd "$ROOT" && cargo test --workspace 2>&1)
PASSING=$(echo "$TEST_OUTPUT" | grep "test result:" | awk '{sum += $4} END {print sum}')
IGNORED=$(echo "$TEST_OUTPUT" | grep "test result:" | awk '{sum += $8} END {print sum}')
TOTAL=$((PASSING + IGNORED))

# Setpoint count from policy.yaml
SETPOINT_TOTAL=$(grep -c '^\s*- id: S' "$ROOT/.control/policy.yaml" 2>/dev/null || echo "0")
SETPOINT_BLOCKING=$(grep -B0 'severity: blocking' "$ROOT/.control/policy.yaml" 2>/dev/null | grep -c 'severity' || echo "0")
SETPOINT_INFO=$((SETPOINT_TOTAL - SETPOINT_BLOCKING))

# Smoke gate
SMOKE_STATUS="unknown"
if (cd "$ROOT" && make smoke >/dev/null 2>&1); then
  SMOKE_STATUS="pass"
else
  SMOKE_STATUS="fail"
fi

# Control audit
AUDIT_STATUS="unknown"
if (cd "$ROOT" && make control-audit >/dev/null 2>&1); then
  AUDIT_STATUS="pass"
else
  AUDIT_STATUS="fail"
fi

# Crate count
CRATES=$(grep -c '"crates/' "$ROOT/Cargo.toml" 2>/dev/null || echo "0")

GENERATED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)

cat > "$STATE_FILE" << EOF
{
  "version": "$VERSION",
  "generated_at": "$GENERATED_AT",
  "tests": {
    "passing": $PASSING,
    "ignored": $IGNORED,
    "total": $TOTAL
  },
  "setpoints": {
    "total": $SETPOINT_TOTAL,
    "blocking": $SETPOINT_BLOCKING,
    "informational": $SETPOINT_INFO
  },
  "gates": {
    "smoke": "$SMOKE_STATUS",
    "control_audit": "$AUDIT_STATUS"
  },
  "phases": {
    "0_scaffold": "complete",
    "1_config": "complete",
    "2_tracker": "complete",
    "3_workspace": "complete",
    "4_orchestrator": "complete",
    "5_agent": "complete",
    "6_observability": "complete",
    "7_integration": "complete",
    "8_oss": "complete",
    "9_cloud": "planned"
  },
  "crates": $CRATES,
  "rust_lines_approx": 6100
}
EOF

echo "=== state.json refreshed ==="
echo "  Version: $VERSION"
echo "  Tests: $PASSING passing, $IGNORED ignored"
echo "  Setpoints: $SETPOINT_TOTAL ($SETPOINT_BLOCKING blocking, $SETPOINT_INFO informational)"
echo "  Smoke: $SMOKE_STATUS"
echo "  Control Audit: $AUDIT_STATUS"
