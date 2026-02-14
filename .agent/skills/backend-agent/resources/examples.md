# Examples

## Full Module Structure

```rust
// src/items/mod.rs
pub mod router;
pub mod service;
pub mod repository;
pub mod models;
pub mod dtos;

// src/items/models.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Item {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

// src/items/repository.rs
use sqlx::SqlitePool;
use uuid::Uuid;
use crate::items::models::Item;

#[derive(Clone)]
pub struct ItemRepository {
    pool: SqlitePool,
}

impl ItemRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, name: String) -> Result<Item, sqlx::Error> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        
        sqlx::query_as!(
            Item,
            r#"
            INSERT INTO items (id, name, created_at)
            VALUES ($1, $2, $3)
            RETURNING id, name, created_at
            "#,
            id, name, created_at
        )
        .fetch_one(&self.pool)
        .await
    }
}

// src/items/router.rs
use axum::{Router, routing::post, extract::State, Json, response::IntoResponse};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_item))
}

async fn create_item(
    State(state): State<AppState>, 
    Json(payload): Json<CreateItemRequest>
) -> impl IntoResponse {
    // ... logic
}
```
