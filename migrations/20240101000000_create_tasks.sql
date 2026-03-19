CREATE TYPE task_type AS ENUM ('webhook', 'hash');
CREATE TYPE task_state AS ENUM ('pending', 'running', 'completed', 'failed');

CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_type task_type NOT NULL,
    state task_state NOT NULL DEFAULT 'pending',
    execute_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    payload JSONB NOT NULL
);

-- Used by the worker to efficiently find claimable tasks
CREATE INDEX idx_tasks_state_execute_at ON tasks (state, execute_at);
CREATE INDEX idx_tasks_task_type ON tasks (task_type);
