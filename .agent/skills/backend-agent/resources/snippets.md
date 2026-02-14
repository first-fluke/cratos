# Code Snippets

## Axum Handler (Get Item)

```rust
use axum::{extract::{Path, State}, http::StatusCode, response::IntoResponse, Json};
use crate::error::AppError;

pub async fn get_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let item = state.service.get_item(id).await?;
    Ok((StatusCode::OK, Json(item)))
}
```

## SQLx Repository Method

```rust
use sqlx::SqlitePool;
use crate::models::Item;

pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Item>, sqlx::Error> {
        sqlx::query_as!(
            Item,
            r#"
            SELECT id, name, created_at
            FROM items
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }
}
```

## Service Method

```rust
use crate::dtos::ItemDto;

impl Service {
    pub async fn get_item(&self, id: Uuid) -> Result<ItemDto, AppError> {
        let item = self.repository.find_by_id(id).await?
            .ok_or(AppError::NotFound("Item not found".into()))?;
        
        Ok(item.into())
    }
}
```

## Parsing JSON Body

```rust
pub async fn create_item(
    State(state): State<AppState>,
    Json(payload): Json<CreateItemRequest>,
) -> Result<impl IntoResponse, AppError> {
    // ...
}
```
