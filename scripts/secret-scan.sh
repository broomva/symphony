#!/usr/bin/env bash
# secret-scan.sh — dependency-free secret scanner for the Symphony repo.
#
# Root cause this guards: the conversation bridge (scripts/conversation-history.py)
# captures raw tool calls into docs/conversations/*.md and commits them to this
# PUBLIC repo. A command that embeds a literal credential would be published
# verbatim — this happened (a Neon Postgres URL + SYMPHONY_API_TOKEN, 2026-03).
#
# Scans the working tree (default) or the staged diff (--staged) for live
# credentials and exits 1 if any are found. Used by:
#   - .githooks/pre-commit           (local, blocks the commit)
#   - .github/workflows/secret-scan  (CI, blocks the PR)
#
# Grep-based on purpose (no gitleaks / python dependency) so it runs everywhere.
#
# Usage:
#   scripts/secret-scan.sh            # scan all tracked files
#   scripts/secret-scan.sh --staged   # scan staged changes only (pre-commit)
#   scripts/secret-scan.sh <path...>  # scan specific files
set -u

MODE="tree"; FILES=()
case "${1:-}" in
  --staged) MODE="staged" ;;
  "")       MODE="tree" ;;
  *)        MODE="files"; FILES=("$@") ;;
esac

# Sensitive KEY=VALUE pattern. The value is a token-shaped opaque blob (20+ chars,
# no '.'/'('/':'), matching scripts/conversation-history.py::_KV_SECRET so the CI
# gate and the bridge redactor agree — this avoids flagging truncated placeholders
# like `sk-ant-api03-...` while still catching real 20+ char credentials.
# (Double-quoted so the pattern may contain both quote chars.)
KV_PATTERN="([A-Za-z0-9_]*(TOKEN|SECRET|PASSWORD|PASSWD|API[_-]?KEY|ACCESS[_-]?KEY|PRIVATE[_-]?KEY|CLIENT[_-]?SECRET|AUTH[_-]?SECRET)[A-Za-z0-9_]*[[:space:]]*[:=][[:space:]]*[\"']?[A-Za-z0-9/_+=~-]{20,})"

# High-signal secret patterns (POSIX extended regex).
PATTERNS=(
  # Credentialed connection strings: scheme://user:password@host
  '(postgres(ql)?|mysql|mongodb(\+srv)?|redis|amqps?|https?|ftp)://[^/[:space:]:@]+:[^/[:space:]@]+@'
  "$KV_PATTERN"
  # CLI-flag credentials: --token <blob>, --api-key <blob>, …
  '\-\-?(token|password|secret|api[-_]?key|auth[-_]?token|access[-_]?key)[=[:space:]]+[A-Za-z0-9/_+=~-]{20,}'
  'AKIA[0-9A-Z]{16}'                  # AWS access key id
  'sk-ant-[A-Za-z0-9_-]{20,}'         # Anthropic
  'sk-[A-Za-z0-9]{40,}'               # OpenAI
  'gh[pousr]_[A-Za-z0-9]{30,}'        # GitHub token
  'github_pat_[A-Za-z0-9_]{40,}'      # GitHub fine-grained PAT
  'xox[baprs]-[A-Za-z0-9-]{10,}'      # Slack
  '-----BEGIN[A-Z ]*PRIVATE KEY-----' # PEM private key
)

# Allowlist — placeholders, local refs, env-var indirections, already-redacted.
ALLOW='REDACTED|localhost|127\.0\.0\.1|0\.0\.0\.0|change-?me|example|placeholder|dummy|sample|test|[Xx]{3,}|your[_-]|YOUR[_-]|NOT_SET|=[[:space:]]*\$|:[[:space:]]*\$|\$\{|\$\(|user:pass@|username:password|<[A-Za-z_]'

list_files() {
  case "$MODE" in
    staged) git diff --cached --name-only --diff-filter=ACM ;;
    files)  printf '%s\n' "${FILES[@]}" ;;
    tree)   git ls-files ;;
  esac
}

redact() {  # redact the secret value in a reported line so findings never print it
  sed -E \
    -e 's#(://[^:]+:)[^@]+(@)#\1[REDACTED]\2#g' \
    -e 's#([:=][[:space:]]*["'"'"']?)[A-Za-z0-9/_+=~.-]{8,}#\1[REDACTED]#g' \
    -e 's#(--?[A-Za-z-]*(token|password|secret|key)[=[:space:]]+)[A-Za-z0-9/_+=~-]{8,}#\1[REDACTED]#gi' \
    -e 's#[A-Za-z0-9/_+=~-]{20,}#[REDACTED]#g'
}

FINDINGS="$(mktemp)"; trap 'rm -f "$FINDINGS"' EXIT

while IFS= read -r f; do
  [ -z "$f" ] && continue
  [ -f "$f" ] || continue
  case "$f" in
    scripts/secret-scan.sh|scripts/test_secret_redaction.py|SECURITY.md|*.lock|*.png|*.jpg|*.jpeg|*.gif|*.pdf|*.ico|*.svg|*.woff|*.woff2|*.ttf) continue ;;
  esac
  grep -Iq . "$f" 2>/dev/null || continue   # -I skips binary files
  for pat in "${PATTERNS[@]}"; do
    grep -nE "$pat" "$f" 2>/dev/null | grep -vE "$ALLOW" | while IFS=: read -r ln rest; do
      printf '  %s:%s: %s\n' "$f" "$ln" "$(printf '%s' "${rest}" | redact | cut -c1-160)"
    done
  done
done < <(list_files) | sort -u > "$FINDINGS"

if [ -s "$FINDINGS" ]; then
  echo "❌ secret-scan: potential live credential(s) detected:"
  cat "$FINDINGS"
  echo ""
  echo "→ Remove the secret, rotate it, and reference an env var / secret manager instead."
  echo "→ False positive? Extend the ALLOW list in scripts/secret-scan.sh."
  exit 1
fi

echo "✅ secret-scan: no live credentials detected (mode: $MODE)"
exit 0
