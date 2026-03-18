#!/usr/bin/env bash
set -euo pipefail

API_URL="${API_URL:-http://localhost:3000}"
WEBHOOK_URL="${WEBHOOK_URL:-https://play.svix.com/in/e_ULeUsyr2O4ucGzX2AchL8g8664G/}"
COUNT="${1:-10}"

echo "Creating $COUNT webhook tasks -> $WEBHOOK_URL"

for i in $(seq 1 "$COUNT"); do
  curl -s -X POST "$API_URL/tasks" \
    -H "Content-Type: application/json" \
    -d "{
      \"task_type\": \"webhook\",
      \"execute_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",
      \"url\": \"$WEBHOOK_URL\",
      \"body\": \"{\\\"event\\\": \\\"test\\\", \\\"index\\\": $i}\"
    }" | jq -r '.id // "error"'
done

echo "Done."
