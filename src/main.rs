mod api;
mod db;
mod models;
mod worker;

use sqlx::postgres::PgPoolOptions;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cmd = std::env::args().nth(1);

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or("postgres://postgres:postgres@localhost/scheduler".into());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let token = CancellationToken::new();

    // Cancel everything on Ctrl+C
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Shutting down...");
        shutdown_token.cancel();
    });

    // If a command is specified, run only that component.
    // Otherwise run both API and worker in the same process.
    match cmd {
        Some(cmd) if cmd == "api" => {
            if let Err(err) = api::run(pool, token).await {
                tracing::error!(error = %err, "API server failed");
            }
        }
        Some(cmd) if cmd == "worker" => {
            worker::run(pool, token).await;
        }
        _ => {
            let worker_handle = worker::run_in_background(pool.clone(), token.clone());
            if let Err(err) = api::run(pool, token).await {
                tracing::error!(error = %err, "API server failed");
            }
            worker_handle.await.ok();
        }
    }
}
