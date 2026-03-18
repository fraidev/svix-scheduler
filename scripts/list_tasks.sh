#!/usr/bin/env bash
set -euo pipefail

API_URL="${API_URL:-http://localhost:3000}"
STATE="${1:-}"

if [ -n "$STATE" ]; then
  curl -s "$API_URL/tasks?state=$STATE" | jq
else
  curl -s "$API_URL/tasks" | jq
fi
