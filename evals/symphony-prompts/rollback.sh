#!/usr/bin/env bash
# EGRI Rollback — restores stimulus-workflow.md from baseline snapshot.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BASELINE="$ROOT/evals/symphony-prompts/baseline/stimulus-workflow.md"
TARGET="$ROOT/stimulus-workflow.md"

if [ ! -f "$BASELINE" ]; then
  echo "ERROR: Baseline not found at $BASELINE"
  exit 1
fi

cp "$BASELINE" "$TARGET"
echo "Rolled back stimulus-workflow.md to baseline snapshot"
echo "Run 'make smoke' to verify the rollback is clean"
