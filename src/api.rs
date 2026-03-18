use axum::{routing::get, Router};
use sqlx::PgPool;

pub async fn run(pool: PgPool) {
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
