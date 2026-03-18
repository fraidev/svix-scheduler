use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;
use sqlx::PgPool;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::db;
use crate::models::TaskType;

pub fn run_in_background(pool: PgPool) -> JoinHandle<()> {
    tokio::spawn({
        async move {
            run(pool).await;
        }
    })
}

pub async fn run(pool: PgPool) {
    let client = reqwest::Client::new();

    loop {
        match db::claim_pending_task(&pool).await {
            Ok(Some(task)) => {
                let result = match task.task_type {
                    TaskType::Webhook => execute_webhook(&client, &task.payload).await,
                    TaskType::Hash => execute_hash(&task.payload).await,
                };
                match result {
                    Ok(()) => {
                        let _ = db::complete_task(&pool, task.id).await;
                    }
                    Err(e) => {
                        eprintln!("Task {} failed: {e}", task.id);
                        let _ = db::fail_task(&pool, task.id).await;
                    }
                }
            }
            Ok(None) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => {
                eprintln!("Error claiming task: {e}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
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

    println!("Webhook {url} responded with status {}", resp.status());
    Ok(())
}

async fn execute_hash(payload: &serde_json::Value) -> Result<(), String> {
    let secret = payload["secret"].as_str().ok_or("missing secret")?.to_owned();

    let encoded = tokio::task::spawn_blocking(move || {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);

        let mut derived = [0u8; 32];
        pbkdf2_hmac::<Sha256>(secret.as_bytes(), &salt, 600_000, &mut derived);

        base64::engine::general_purpose::STANDARD.encode(derived)
    })
    .await
    .map_err(|e| e.to_string())?;

    println!("Hash result: {encoded}");
    Ok(())
}
