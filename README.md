# Svix Scheduler

A task scheduling service with two task types:

- **Webhook** — POST to a URL with a given body, log the response status code.
- **Hash** — PBKDF2-HMAC-SHA256 with a random salt for 600,000 iterations, log the base64-encoded derived value.

Tasks are persisted in PostgreSQL and executed at or after their scheduled time. Multiple workers can run in parallel — each task is processed exactly once using `SELECT ... FOR UPDATE SKIP LOCKED`.

## Prerequisites

- Rust
- Docker

## Quick start

### 1. Start PostgreSQL

```bash
docker run -d --name scheduler-db \
  -p 5432:5432 \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=scheduler \
  postgres:18.3
```

### 2. Build and run

```bash
cargo run
```

This starts both the API (on port 3000) and a worker in the same process. Migrations run automatically on startup.

### 3. Create some tasks

```bash
# 10 webhook tasks
./scripts/create_webhooks.sh 10

# 10 hash tasks
./scripts/create_hashes.sh 10

# Check progress
./scripts/task_stats.sh
```

## Architecture

```
┌──────────┐       ┌────────────┐       ┌──────────────┐
│  Client   │──────▶│  API (HTTP) │──────▶│  PostgreSQL   │
└──────────┘       └────────────┘       └──────┬───────┘
                                               │
                                    ┌──────────┴───────────┐
                                    │    Worker(s) poll     │
                                    │  FOR UPDATE SKIP LOCKED│
                                    └──────────────────────┘
```

### Project structure

```
src/
  main.rs    — Entrypoint, DB pool, migrations, signal handling
  api.rs     — Axum HTTP handlers (CRUD for tasks)
  worker.rs  — Poll loop, webhook delivery, PBKDF2 hashing
  db.rs      — SQL queries (create, get, list, delete, claim, complete, fail)
  models.rs  — Task, TaskType, TaskState
migrations/
  20240101000000_create_tasks.sql
scripts/
  create_webhooks.sh  — Bulk-create webhook tasks
  create_hashes.sh    — Bulk-create hash tasks
  list_tasks.sh       — List tasks (optionally filtered by state)
  task_stats.sh       — Show task counts per state
  reset_db.sh         — Drop and recreate the database
```

## Running modes

The binary accepts an optional argument: `api`, `worker`, or no argument (runs both).

### Default — API + worker in one process

```bash
cargo run
```

### Independent scaling

Run the API separately:

```bash
cargo run -- api
```

Run multiple workers in separate terminals:

```bash
cargo run -- worker  # terminal 1
cargo run -- worker  # terminal 2
cargo run -- worker  # terminal 3
```

Each worker independently polls for tasks. The `FOR UPDATE SKIP LOCKED` pattern ensures no task is processed twice.

## Configuration

| Variable       | Default                                            | Description                      |
| -------------- | -------------------------------------------------- | -------------------------------- |
| `DATABASE_URL` | `postgres://postgres:postgres@localhost/scheduler`  | PostgreSQL connection string     |
| `RUST_LOG`     | (none)                                             | Log level (`info`, `debug`, etc) |

## API reference

### Create a task

**POST** `/tasks`

Webhook:
```bash
curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "task_type": "webhook",
    "execute_at": "2026-03-18T12:00:00Z",
    "url": "https://play.svix.com/in/e_example/",
    "body": "{\"event\": \"test\"}"
  }'
```

Hash:
```bash
curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "task_type": "hash",
    "execute_at": "2026-03-18T12:00:00Z",
    "secret": "mysecret"
  }'
```

Returns `201` with the created task (including its `id`).

### List tasks

**GET** `/tasks`

```bash
# All tasks
curl http://localhost:3000/tasks

# Filter by state and/or type
curl "http://localhost:3000/tasks?state=pending"
curl "http://localhost:3000/tasks?task_type=webhook"
curl "http://localhost:3000/tasks?state=completed&task_type=hash"
```

### Get a task

**GET** `/tasks/{id}`

```bash
curl http://localhost:3000/tasks/550e8400-e29b-41d4-a716-446655440000
```

Returns `200` with the task or `404`.

### Delete a task

**DELETE** `/tasks/{id}`

```bash
curl -X DELETE http://localhost:3000/tasks/550e8400-e29b-41d4-a716-446655440000
```

Returns `204` on success or `404`.

## Scripts

All scripts accept `API_URL` env (defaults to `http://localhost:3000`).

```bash
# Create 20 webhook tasks (set WEBHOOK_URL to override the target)
./scripts/create_webhooks.sh 20

# Create 5 hash tasks
./scripts/create_hashes.sh 5

# List all pending tasks
./scripts/list_tasks.sh pending

# Show counts by state
./scripts/task_stats.sh

# Reset the database (requires restart to re-run migrations)
./scripts/reset_db.sh
```

## Design decisions

- **Single binary, multiple modes** — `api`, `worker`, or both. Simpler than two binaries, same scaling flexibility.
- **PostgreSQL** — Persistent, supports horizontal scaling via row-level locking, handles the filtering/listing requirements naturally.
- **`FOR UPDATE SKIP LOCKED`** — Workers atomically claim tasks without conflicts. No external coordination needed.
- **`spawn_blocking` for hashing** — 600k PBKDF2 iterations is CPU-heavy; offloaded to a blocking thread to avoid stalling the async runtime.
- **Graceful shutdown** — Ctrl+C cancels a shared token. The API drains in-flight requests, and the worker finishes its current task before exiting.
- **JSONB payload** — Each task type stores its own fields (`url`+`body` or `secret`) without extra columns.
- **Logging** — Structured logging with `tracing` for better observability and debugging.
- **Migrations on startup** — Ensures the database schema is always up-to-date without manual intervention. In production, a separate migration step might be preferable.
- **Adaptive backoff** — The worker poll loop uses [`adaptive-backoff`](https://github.com/brhoades/adaptive-backoff) to self-tune its polling interval. Slows down after failures (up to 30s), recovers quickly after successes.
