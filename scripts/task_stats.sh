#!/usr/bin/env bash
set -euo pipefail

API_URL="${API_URL:-http://localhost:3000}"

echo "Task counts by state:"
for state in pending running completed failed; do
  count=$(curl -s "$API_URL/tasks?state=$state" | jq 'length')
  printf "  %-10s %s\n" "$state" "$count"
done
