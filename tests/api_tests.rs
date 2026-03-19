use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::PgPool;
use svix_scheduler::{api, db, models::*};
use tower::ServiceExt;

async fn setup() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or("postgres://postgres:postgres@localhost/scheduler".into());

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Clean up before each test
    sqlx::query("DELETE FROM tasks")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

fn post_json(uri: &str, json: serde_json::Value) -> Request<Body> {
    Request::post(uri)
        .header("Content-Type", "application/json")
        .body(Body::from(json.to_string()))
        .unwrap()
}

fn empty_request(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// -- API Integration Tests --

#[tokio::test]
async fn create_webhook_task() {
    let pool = setup().await;
    let app = api::router(pool);

    let resp = app
        .oneshot(post_json(
            "/tasks",
            serde_json::json!({
                "task_type": "webhook",
                "execute_at": "2099-01-01T00:00:00Z",
                "url": "https://example.com/hook",
                "body": "{\"test\": true}"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = body_json(resp).await;
    assert_eq!(body["task_type"], "webhook");
    assert_eq!(body["state"], "pending");
    assert!(body["id"].as_str().is_some());
}

#[tokio::test]
async fn create_hash_task() {
    let pool = setup().await;
    let app = api::router(pool);

    let resp = app
        .oneshot(post_json(
            "/tasks",
            serde_json::json!({
                "task_type": "hash",
                "execute_at": "2099-01-01T00:00:00Z",
                "secret": "mysecret"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = body_json(resp).await;
    assert_eq!(body["task_type"], "hash");
    assert_eq!(body["state"], "pending");
}

#[tokio::test]
async fn create_webhook_empty_url_rejected() {
    let pool = setup().await;
    let app = api::router(pool);

    let resp = app
        .oneshot(post_json(
            "/tasks",
            serde_json::json!({
                "task_type": "webhook",
                "execute_at": "2099-01-01T00:00:00Z",
                "url": "",
                "body": "{}"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_hash_empty_secret_rejected() {
    let pool = setup().await;
    let app = api::router(pool);

    let resp = app
        .oneshot(post_json(
            "/tasks",
            serde_json::json!({
                "task_type": "hash",
                "execute_at": "2099-01-01T00:00:00Z",
                "secret": ""
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_task_by_id() {
    let pool = setup().await;
    let task = db::create_task(
        &pool,
        TaskType::Hash,
        chrono::Utc::now(),
        serde_json::json!({"secret": "s"}),
    )
    .await
    .unwrap();

    let app = api::router(pool);
    let resp = app
        .oneshot(empty_request("GET", &format!("/tasks/{}", task.id)))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["id"], task.id.to_string());
}

#[tokio::test]
async fn get_task_not_found() {
    let pool = setup().await;
    let app = api::router(pool);

    let resp = app
        .oneshot(empty_request(
            "GET",
            "/tasks/00000000-0000-0000-0000-000000000000",
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_task_returns_no_content() {
    let pool = setup().await;
    let task = db::create_task(
        &pool,
        TaskType::Webhook,
        chrono::Utc::now(),
        serde_json::json!({"url": "https://example.com", "body": "{}"}),
    )
    .await
    .unwrap();

    let app = api::router(pool);
    let resp = app
        .oneshot(empty_request("DELETE", &format!("/tasks/{}", task.id)))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn delete_task_not_found() {
    let pool = setup().await;
    let app = api::router(pool);

    let resp = app
        .oneshot(empty_request(
            "DELETE",
            "/tasks/00000000-0000-0000-0000-000000000000",
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_tasks_filter_by_state() {
    let pool = setup().await;

    db::create_task(
        &pool,
        TaskType::Webhook,
        chrono::Utc::now(),
        serde_json::json!({"url": "https://example.com", "body": "{}"}),
    )
    .await
    .unwrap();

    let hash = db::create_task(
        &pool,
        TaskType::Hash,
        chrono::Utc::now(),
        serde_json::json!({"secret": "s"}),
    )
    .await
    .unwrap();
    db::complete_task(&pool, hash.id).await.unwrap();

    let app = api::router(pool);
    let resp = app
        .oneshot(empty_request("GET", "/tasks?state=pending"))
        .await
        .unwrap();

    let body = body_json(resp).await;
    let tasks = body.as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["task_type"], "webhook");
}

#[tokio::test]
async fn list_tasks_filter_by_type() {
    let pool = setup().await;

    db::create_task(
        &pool,
        TaskType::Webhook,
        chrono::Utc::now(),
        serde_json::json!({"url": "https://example.com", "body": "{}"}),
    )
    .await
    .unwrap();

    db::create_task(
        &pool,
        TaskType::Hash,
        chrono::Utc::now(),
        serde_json::json!({"secret": "s"}),
    )
    .await
    .unwrap();

    let app = api::router(pool);
    let resp = app
        .oneshot(empty_request("GET", "/tasks?task_type=hash"))
        .await
        .unwrap();

    let body = body_json(resp).await;
    let tasks = body.as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["task_type"], "hash");
}

// -- Claim Concurrency Tests --

#[tokio::test]
async fn claim_processes_each_task_once() {
    let pool = setup().await;

    for _ in 0..10 {
        db::create_task(
            &pool,
            TaskType::Hash,
            chrono::Utc::now(),
            serde_json::json!({"secret": "s"}),
        )
        .await
        .unwrap();
    }

    // Simulate 10 concurrent workers claiming tasks
    let mut handles = Vec::new();
    for _ in 0..10 {
        let pool = pool.clone();
        handles.push(tokio::spawn(
            async move { db::claim_pending_task(&pool).await },
        ));
    }

    let mut claimed_ids = std::collections::HashSet::new();
    for handle in handles {
        if let Ok(Some(task)) = handle.await.unwrap() {
            assert!(
                claimed_ids.insert(task.id),
                "Task {} was claimed twice!",
                task.id
            );
            assert_eq!(task.state, TaskState::Running);
        }
    }

    assert_eq!(claimed_ids.len(), 10);

    let remaining = db::list_tasks(&pool, Some(TaskState::Pending), None)
        .await
        .unwrap();
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn claim_skips_future_tasks() {
    let pool = setup().await;

    db::create_task(
        &pool,
        TaskType::Hash,
        chrono::Utc::now() + chrono::Duration::hours(24),
        serde_json::json!({"secret": "s"}),
    )
    .await
    .unwrap();

    let claimed = db::claim_pending_task(&pool).await.unwrap();
    assert!(claimed.is_none());
}

#[tokio::test]
async fn claim_returns_none_when_empty() {
    let pool = setup().await;

    let claimed = db::claim_pending_task(&pool).await.unwrap();
    assert!(claimed.is_none());
}
