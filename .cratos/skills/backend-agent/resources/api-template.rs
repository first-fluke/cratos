use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

// --- DOMAIN MODELS ---
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Widget {
    pub id: Uuid,
    pub name: String,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateWidgetRequest {
    pub name: String,
    pub quantity: i32,
}

// --- ERROR HANDLING ---
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Widget not found")]
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "Widget not found".to_string()),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

// --- REPOSITORY LAYER ---
#[derive(Clone)]
pub struct WidgetRepository {
    pool: SqlitePool,
}

impl WidgetRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, name: String, quantity: i32) -> Result<Widget, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query_as!(
            Widget,
            r#"
            INSERT INTO widgets (id, name, quantity)
            VALUES ($1, $2, $3)
            RETURNING id, name, "quantity: i32"
            "#,
            id,
            name,
            quantity
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Widget>, sqlx::Error> {
        sqlx::query_as!(
            Widget,
            r#"
            SELECT id, name, "quantity: i32"
            FROM widgets
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }
}

// --- SERVICE LAYER ---
#[derive(Clone)]
pub struct WidgetService {
    repo: WidgetRepository,
}

impl WidgetService {
    pub fn new(repo: WidgetRepository) -> Self {
        Self { repo }
    }

    pub async fn create_widget(&self, req: CreateWidgetRequest) -> Result<Widget, AppError> {
        Ok(self.repo.create(req.name, req.quantity).await?)
    }

    pub async fn get_widget(&self, id: Uuid) -> Result<Widget, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or(AppError::NotFound)
    }
}

// --- APP STATE ---
#[derive(Clone)]
pub struct AppState {
    pub widget_service: WidgetService,
}

// --- ROUTER LAYER ---
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/widgets", post(create_widget))
        .route("/widgets/:id", get(get_widget))
        .with_state(state)
}

async fn create_widget(
    State(state): State<AppState>,
    Json(payload): Json<CreateWidgetRequest>,
) -> Result<impl IntoResponse, AppError> {
    let widget = state.widget_service.create_widget(payload).await?;
    Ok((StatusCode::CREATED, Json(widget)))
}

async fn get_widget(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let widget = state.widget_service.get_widget(id).await?;
    Ok(Json(widget))
}

// --- MAIN (Example) ---
// #[tokio::main]
// async fn main() {
//     let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
//     let repo = WidgetRepository::new(pool);
//     let service = WidgetService::new(repo);
//     let state = AppState { widget_service: service };
//     
//     let app = router(state);
//     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
//     axum::serve(listener, app).await.unwrap();
// }
