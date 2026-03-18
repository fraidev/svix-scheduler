use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::models::{Task, TaskState, TaskType};

pub async fn run(pool: PgPool, token: tokio_util::sync::CancellationToken) {
    let app = Router::new()
        .route("/tasks", post(create_task))
        .route("/tasks", get(list_tasks))
        .route("/tasks/{id}", get(get_task))
        .route("/tasks/{id}", delete(delete_task))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(token.cancelled_owned())
        .await
        .unwrap();
}

#[derive(Deserialize)]
#[serde(tag = "task_type", rename_all = "lowercase")]
enum CreateTaskRequest {
    Webhook {
        execute_at: chrono::DateTime<chrono::Utc>,
        url: String,
        body: String,
    },
    Hash {
        execute_at: chrono::DateTime<chrono::Utc>,
        secret: String,
    },
}

async fn create_task(
    State(pool): State<PgPool>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<Task>), (StatusCode, String)> {
    let (task_type, execute_at, payload) = match req {
        CreateTaskRequest::Webhook {
            execute_at,
            url,
            body,
        } => (
            TaskType::Webhook,
            execute_at,
            serde_json::json!({ "url": url, "body": body }),
        ),
        CreateTaskRequest::Hash { execute_at, secret } => (
            TaskType::Hash,
            execute_at,
            serde_json::json!({ "secret": secret }),
        ),
    };

    let task = db::create_task(&pool, task_type, execute_at, payload)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(task)))
}

#[derive(Deserialize)]
struct ListQuery {
    state: Option<TaskState>,
    task_type: Option<TaskType>,
}

async fn list_tasks(
    State(pool): State<PgPool>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<Task>>, (StatusCode, String)> {
    let tasks = db::list_tasks(&pool, query.state, query.task_type)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(tasks))
}

async fn get_task(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Task>, StatusCode> {
    db::get_task(&pool, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn delete_task(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let deleted = db::delete_task(&pool, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
