#!/bin/bash
# Symphony managed-mode startup script.
# Fetches WORKFLOW.md from control plane if SYMPHONY_CLOUD_CONFIG_URL is set,
# otherwise uses the local file.

set -e

if [ -n "$SYMPHONY_CLOUD_CONFIG_URL" ]; then
  echo "Fetching workflow from control plane..."
  curl -sfH "Authorization: Bearer ${SYMPHONY_CLOUD_TOKEN:-}" \
    "$SYMPHONY_CLOUD_CONFIG_URL" > /app/WORKFLOW.md \
    || { echo "Failed to fetch workflow config"; exit 1; }
fi

exec symphony start --port "${PORT:-8080}" /app/WORKFLOW.md
