#!/usr/bin/env bash
# EGRI Constraint Checker — validates that the current state is safe for evaluation.
# Exit 0 = all constraints pass, Exit 1 = constraint violation.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORKFLOW="$ROOT/stimulus-workflow.md"
FAIL=0

pass() { echo "  ✓ $1"; }
fail() { echo "  ✗ $1"; FAIL=1; }

echo "=== EGRI Constraint Check ==="

# 1. make smoke must pass
echo ""
echo "--- Gate: make smoke ---"
if (cd "$ROOT" && make smoke >/dev/null 2>&1); then
  pass "make smoke passes"
else
  fail "make smoke failed"
fi

# 2. stimulus-workflow.md must have valid YAML front matter
echo ""
echo "--- Workflow: valid front matter ---"
if [ -f "$WORKFLOW" ]; then
  if head -1 "$WORKFLOW" | grep -q '^---'; then
    pass "stimulus-workflow.md has front matter"
  else
    fail "stimulus-workflow.md missing front matter (first line must be ---)"
  fi
else
  fail "stimulus-workflow.md not found"
fi

# 3. Prompt must include {{ issue.identifier }} and {{ issue.title }}
echo ""
echo "--- Workflow: required template variables ---"
if [ -f "$WORKFLOW" ]; then
  if grep -q '{{ issue.identifier }}' "$WORKFLOW"; then
    pass "Contains {{ issue.identifier }}"
  else
    fail "Missing {{ issue.identifier }}"
  fi

  if grep -q '{{ issue.title }}' "$WORKFLOW"; then
    pass "Contains {{ issue.title }}"
  else
    fail "Missing {{ issue.title }}"
  fi
else
  fail "stimulus-workflow.md not found (cannot check template variables)"
fi

# 4. Prompt must include control metalayer instructions
echo ""
echo "--- Workflow: consciousness stack directives ---"
if [ -f "$WORKFLOW" ]; then
  if grep -qi 'CLAUDE.md' "$WORKFLOW"; then
    pass "References CLAUDE.md (governance)"
  else
    fail "Missing CLAUDE.md reference (consciousness substrate 2)"
  fi

  if grep -qi 'AGENTS.md' "$WORKFLOW"; then
    pass "References AGENTS.md (boundaries)"
  else
    fail "Missing AGENTS.md reference (consciousness substrate 2)"
  fi

  if grep -qi 'conversations' "$WORKFLOW"; then
    pass "References conversation history (episodic memory)"
  else
    fail "Missing conversation history reference (consciousness substrate 3)"
  fi
else
  fail "stimulus-workflow.md not found (cannot check consciousness directives)"
fi

echo ""
if [ "$FAIL" -eq 0 ]; then
  echo "=== CONSTRAINT CHECK PASS ==="
else
  echo "=== CONSTRAINT CHECK FAIL ==="
  exit 1
fi
