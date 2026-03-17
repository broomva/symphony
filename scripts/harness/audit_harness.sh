#!/usr/bin/env bash
# Harness audit: validates that the foundational harness practices are in place.
# Exit 0 = pass, Exit 1 = fail with details.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FAIL=0

pass() { echo "  ✓ $1"; }
fail() { echo "  ✗ $1"; FAIL=1; }

echo "=== Harness Audit ==="

# 1. Required governance files
echo ""
echo "--- Governance Files ---"
for f in AGENTS.md CLAUDE.md CONTROL.md Makefile; do
  [ -f "$ROOT/$f" ] && pass "$f exists" || fail "$f missing"
done

# 2. Makefile has smoke target
echo ""
echo "--- Makefile Targets ---"
grep -q '^smoke:' "$ROOT/Makefile" && pass "Makefile has 'smoke' target" || fail "Makefile missing 'smoke' target"

# 3. Pre-commit hook exists and is executable
echo ""
echo "--- Pre-Commit Hook ---"
if [ -f "$ROOT/.githooks/pre-commit" ]; then
  pass ".githooks/pre-commit exists"
  [ -x "$ROOT/.githooks/pre-commit" ] && pass "pre-commit is executable" || fail "pre-commit is not executable"
else
  fail ".githooks/pre-commit missing"
fi

# 4. CI workflow exists
echo ""
echo "--- CI ---"
if ls "$ROOT/.github/workflows/"*.yml >/dev/null 2>&1 || ls "$ROOT/.github/workflows/"*.yaml >/dev/null 2>&1; then
  pass "CI workflow found in .github/workflows/"
else
  # Also check for railway.toml or Dockerfile as deployment config
  if [ -f "$ROOT/railway.toml" ] || [ -f "$ROOT/Dockerfile" ]; then
    pass "Deployment config found (railway.toml / Dockerfile)"
  else
    fail "No CI workflow or deployment config found"
  fi
fi

# 5. All docs/ files have YAML frontmatter
echo ""
echo "--- Documentation Frontmatter ---"
DOCS_FAIL=0
while IFS= read -r -d '' mdfile; do
  if ! head -1 "$mdfile" | grep -q '^---$'; then
    fail "Missing frontmatter: ${mdfile#$ROOT/}"
    DOCS_FAIL=1
  fi
done < <(find "$ROOT/docs" -name '*.md' -print0 2>/dev/null)
[ "$DOCS_FAIL" -eq 0 ] && pass "All docs/*.md files have YAML frontmatter"

# 6. CONTROL.md deviation log not stale (last entry within 90 days)
echo ""
echo "--- CONTROL.md Deviation Log ---"
if [ -f "$ROOT/CONTROL.md" ]; then
  LAST_DATE=$(grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' "$ROOT/CONTROL.md" | tail -1)
  if [ -n "$LAST_DATE" ]; then
    LAST_TS=$(date -j -f "%Y-%m-%d" "$LAST_DATE" "+%s" 2>/dev/null || date -d "$LAST_DATE" "+%s" 2>/dev/null || echo "0")
    NOW_TS=$(date "+%s")
    DAYS_AGO=$(( (NOW_TS - LAST_TS) / 86400 ))
    if [ "$DAYS_AGO" -lt 90 ]; then
      pass "Deviation log active (last entry: $LAST_DATE, ${DAYS_AGO}d ago)"
    else
      fail "Deviation log stale (last entry: $LAST_DATE, ${DAYS_AGO}d ago)"
    fi
  else
    fail "No dates found in CONTROL.md"
  fi
else
  fail "CONTROL.md not found"
fi

echo ""
if [ "$FAIL" -eq 0 ]; then
  echo "=== HARNESS AUDIT PASS ==="
else
  echo "=== HARNESS AUDIT FAIL ==="
  exit 1
fi
