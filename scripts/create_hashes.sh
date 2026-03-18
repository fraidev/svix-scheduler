#!/usr/bin/env bash
set -euo pipefail

API_URL="${API_URL:-http://localhost:3000}"
COUNT="${1:-10}"

echo "Creating $COUNT hash tasks"

for i in $(seq 1 "$COUNT"); do
  curl -s -X POST "$API_URL/tasks" \
    -H "Content-Type: application/json" \
    -d "{
      \"task_type\": \"hash\",
      \"execute_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",
      \"secret\": \"secret-$i-$(openssl rand -hex 8)\"
    }" | jq -r '.id // "error"'
done

echo "Done."
