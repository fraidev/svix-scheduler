use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub token: CancellationToken,
}
