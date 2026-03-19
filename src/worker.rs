use adaptive_backoff::prelude::*;
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;
use sqlx::PgPool;
use std::time::Duration;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::db;
use crate::models::{TaskState, TaskType};
use crate::state::AppState;

const MAX_DB_RETRIES: usize = 5;
const TASK_BACKOFF_MAX: Duration = Duration::from_secs(30);
const DB_BACKOFF_MAX: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const BACKOFF_FACTOR: f64 = 1.1;
const HASH_ITERATIONS: u32 = 600_000;

pub fn run_in_background(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        run(state).await;
    })
}

pub async fn run(state: AppState) {
    tracing::info!("Worker started");
    let client = reqwest::Client::new();

    loop {
        if state.token.is_cancelled() {
            tracing::info!("Worker shutting down");
            return;
        }

        match db::claim_pending_task(&state.pool).await {
            Ok(Some(task)) => {
                execute_task(&client, &state.pool, &task).await;
            }
            Ok(None) => {
                tokio::select! {
                    () = tokio::time::sleep(POLL_INTERVAL) => {}
                    () = state.token.cancelled() => {}
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Error claiming task");
                tokio::select! {
                    () = tokio::time::sleep(POLL_INTERVAL) => {}
                    () = state.token.cancelled() => {}
                }
            }
        }
    }
}

async fn execute_task(client: &reqwest::Client, pool: &PgPool, task: &crate::models::Task) {
    let mut backoff = new_backoff(TASK_BACKOFF_MAX);

    loop {
        let result = match task.task_type {
            TaskType::Webhook => execute_webhook(client, &task.payload).await,
            TaskType::Hash => execute_hash(&task.payload).await,
        };

        match result {
            Ok(()) => {
                set_state_with_retry(pool, task.id, TaskState::Completed).await;
                return;
            }
            Err(e) => {
                let wait = backoff.fail();
                tracing::warn!(task_id = %task.id, error = %e, backoff_ms = wait.as_millis(), "Task failed, retrying");

                if wait >= TASK_BACKOFF_MAX {
                    tracing::error!(task_id = %task.id, "Giving up after max backoff");
                    set_state_with_retry(pool, task.id, TaskState::Failed).await;
                    return;
                }

                tokio::time::sleep(wait).await;
            }
        }
    }
}

async fn set_state_with_retry(pool: &PgPool, id: Uuid, state: TaskState) {
    let mut backoff = new_backoff(DB_BACKOFF_MAX);

    for attempt in 1..=MAX_DB_RETRIES {
        let result = match state {
            TaskState::Completed => db::complete_task(pool, id).await,
            TaskState::Failed => db::fail_task(pool, id).await,
            _ => unreachable!(),
        };

        match result {
            Ok(()) => return,
            Err(e) => {
                let wait = backoff.fail();
                tracing::error!(
                    task_id = %id,
                    error = %e,
                    attempt,
                    max = MAX_DB_RETRIES,
                    "Failed to set task state to {state:?}, retrying"
                );
                tokio::time::sleep(wait).await;
            }
        }
    }

    tracing::error!(task_id = %id, "Exhausted {MAX_DB_RETRIES} retries setting task state to {state:?}");
}

fn new_backoff(max: Duration) -> Adaptive<ExponentialBackoff> {
    ExponentialBackoffBuilder::default()
        .factor(BACKOFF_FACTOR)
        .min(POLL_INTERVAL)
        .max(max)
        .adaptive()
        .build()
        .unwrap()
}

async fn execute_webhook(
    client: &reqwest::Client,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let url = payload["url"].as_str().ok_or("missing url")?;
    let body = payload["body"].as_str().ok_or("missing body")?;

    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(body.to_owned())
        .send()
        .await
        .map_err(|e| e.to_string())?;

    // TODO: Maybe retry on 5xx
    let status = resp.status();
    tracing::info!(url, status = %status, "Webhook delivered");

    Ok(())
}

async fn execute_hash(payload: &serde_json::Value) -> Result<(), String> {
    let secret = payload["secret"]
        .as_str()
        .ok_or("missing secret")?
        .to_owned();

    let encoded = tokio::task::spawn_blocking(move || {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);

        let mut derived = [0u8; 32];
        pbkdf2_hmac::<Sha256>(secret.as_bytes(), &salt, HASH_ITERATIONS, &mut derived);

        base64::engine::general_purpose::STANDARD.encode(derived)
    })
    .await
    .map_err(|e| e.to_string())?;

    tracing::info!(result = %encoded, "Hash computed");
    Ok(())
}
