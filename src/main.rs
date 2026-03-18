mod api;
mod db;
mod models;
mod worker;

use sqlx::postgres::PgPoolOptions;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
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

    let worker_handle = worker::run_in_background(pool.clone(), token.clone());

    // Cancel everything on Ctrl+C
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("Shutting down...");
        shutdown_token.cancel();
    });

    api::run(pool, token).await;

    // Wait for the worker to finish its current task
    worker_handle.await.ok();
}
