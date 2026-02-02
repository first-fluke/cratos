# Rust 코드 스니펫

## Axum 라우터

```rust
use axum::{Router, routing::{get, post}, Json, extract::State};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/messages", post(handle_message))
        .with_state(state)
}

async fn health_check() -> &'static str {
    "OK"
}

async fn handle_message(
    State(state): State<AppState>,
    Json(payload): Json<MessageRequest>,
) -> Result<Json<MessageResponse>, AppError> {
    let result = state.service.process(payload).await?;
    Ok(Json(result))
}
```

## 에러 타입 정의

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Self::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error"),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.as_str()),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Unknown error"),
        };
        (status, message).into_response()
    }
}
```

## sqlx 쿼리

```rust
use sqlx::{PgPool, FromRow};

#[derive(Debug, FromRow)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn find_user(pool: &PgPool, id: i64) -> Result<User, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"SELECT id, name, created_at FROM users WHERE id = $1"#,
        id
    )
    .fetch_one(pool)
    .await
}

pub async fn create_user(pool: &PgPool, name: &str) -> Result<User, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"INSERT INTO users (name) VALUES ($1) RETURNING id, name, created_at"#,
        name
    )
    .fetch_one(pool)
    .await
}
```

## Tracing 설정

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cratos=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

// 함수에 적용
#[tracing::instrument(skip(pool), err)]
pub async fn process(pool: &PgPool, input: &str) -> Result<Output, Error> {
    tracing::info!("Processing input");
    // ...
}
```

## 테스트

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handler() {
        let state = AppState::default();
        let result = handle_request(state, input).await;
        assert!(result.is_ok());
    }

    #[sqlx::test]
    async fn test_db_query(pool: PgPool) {
        let user = create_user(&pool, "test").await.unwrap();
        assert_eq!(user.name, "test");
    }
}
```
