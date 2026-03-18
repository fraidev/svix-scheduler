#!/usr/bin/env bash
set -euo pipefail

CONTAINER="${CONTAINER_NAME:-scheduler-db}"

echo "Dropping and recreating scheduler database..."

docker exec "$CONTAINER" psql -U postgres -c "DROP DATABASE IF EXISTS scheduler;"
docker exec "$CONTAINER" psql -U postgres -c "CREATE DATABASE scheduler;"

echo "Done. Restart the app to re-run migrations."
