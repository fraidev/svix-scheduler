mod api;
mod db;
mod models;

use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or("postgres://postgres:postgres@localhost/scheduler".into());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    tokio::spawn({
        let pool = pool.clone();
        async move {
            // TODO: implement worker loop
        }
    });

    api::run(pool).await;
}
