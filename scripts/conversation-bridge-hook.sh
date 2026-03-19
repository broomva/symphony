#!/bin/bash
set -e
cat > /dev/null 2>&1 || true

STAMP_FILE="${HOME}/.cache/broomva-bridge-stamp"
COOLDOWN=120
LOG_FILE="${HOME}/.cache/broomva-bridge.log"

if [ -f "$STAMP_FILE" ]; then
  if [ "$(uname)" = "Darwin" ]; then
    last_run=$(stat -f %m "$STAMP_FILE" 2>/dev/null || echo 0)
  else
    last_run=$(stat -c %Y "$STAMP_FILE" 2>/dev/null || echo 0)
  fi
  now=$(date +%s)
  elapsed=$((now - last_run))
  if [ "$elapsed" -lt "$COOLDOWN" ]; then
    exit 0
  fi
fi

mkdir -p "$(dirname "$STAMP_FILE")"
touch "$STAMP_FILE"

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRIDGE="$PROJECT_ROOT/scripts/conversation-history.py"

if [ -f "$BRIDGE" ] && command -v python3 >/dev/null 2>&1; then
  (cd "$PROJECT_ROOT" && python3 "$BRIDGE" >> "$LOG_FILE" 2>&1) &
  disown
fi
exit 0
