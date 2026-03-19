use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Task, TaskState, TaskType};

pub async fn create_task(
    pool: &PgPool,
    task_type: TaskType,
    execute_at: chrono::DateTime<chrono::Utc>,
    payload: serde_json::Value,
) -> Result<Task, sqlx::Error> {
    sqlx::query_as(
        "INSERT INTO tasks (task_type, execute_at, payload) \
         VALUES ($1, $2, $3) \
         RETURNING *",
    )
    .bind(task_type)
    .bind(execute_at)
    .bind(sqlx::types::Json(payload))
    .fetch_one(pool)
    .await
}

pub async fn get_task(pool: &PgPool, id: Uuid) -> Result<Option<Task>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_tasks(
    pool: &PgPool,
    state: Option<TaskState>,
    task_type: Option<TaskType>,
) -> Result<Vec<Task>, sqlx::Error> {
    sqlx::query_as(
        "SELECT * FROM tasks \
         WHERE ($1::task_state IS NULL OR state = $1) \
         AND ($2::task_type IS NULL OR task_type = $2) \
         ORDER BY created_at DESC",
    )
    .bind(state)
    .bind(task_type)
    .fetch_all(pool)
    .await
}

pub async fn delete_task(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Atomically claim one pending task that's ready to execute.
/// Uses `FOR UPDATE SKIP LOCKED` so multiple workers don't compete.
pub async fn claim_pending_task(pool: &PgPool) -> Result<Option<Task>, sqlx::Error> {
    sqlx::query_as(
        "UPDATE tasks SET state = 'running' \
         WHERE id = ( \
             SELECT id FROM tasks \
             WHERE state = 'pending' AND execute_at <= now() \
             ORDER BY execute_at ASC \
             LIMIT 1 \
             FOR UPDATE SKIP LOCKED \
         ) \
         RETURNING *",
    )
    .fetch_optional(pool)
    .await
}

pub async fn complete_task(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE tasks SET state = 'completed' WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn fail_task(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE tasks SET state = 'failed' WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
