use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "task_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Webhook,
    Hash,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "task_state", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Task {
    pub id: Uuid,
    pub task_type: TaskType,
    pub state: TaskState,
    pub execute_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub payload: sqlx::types::Json<serde_json::Value>,
}
