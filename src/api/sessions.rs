//! Sessions API endpoints
//!
//! POST   /api/v1/sessions              - Create a new session
//! GET    /api/v1/sessions              - List sessions (owned by requester)
//! GET    /api/v1/sessions/:id          - Get session details
//! DELETE /api/v1/sessions/:id          - Delete a session
//! POST   /api/v1/sessions/:id/messages - Send a message to a session
//! POST   /api/v1/sessions/:id/cancel   - Cancel active execution

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use cratos_core::session_manager::{SessionManager, SessionSummary};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use super::config::ApiResponse;
use crate::middleware::auth::{require_scope, RequireAuth};
use cratos_core::auth::Scope;

/// Shared session manager state.
#[derive(Clone)]
pub struct SessionState {
    manager: Arc<SessionManager>,
}

impl SessionState {
    /// Create a new session state.
    pub fn new() -> Self {
        Self {
            manager: Arc::new(SessionManager::new()),
        }
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to create a session.
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    /// Optional display name
    pub name: Option<String>,
}

/// Request to send a message.
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    /// Message text
    pub text: String,
}

/// Response from sending a message.
#[derive(Debug, serde::Serialize)]
pub struct SendMessageResponse {
    /// Whether execution started immediately
    pub started: bool,
    /// Position in queue (0 if started immediately)
    pub queue_position: usize,
}

/// List sessions owned by the requester.
async fn list_sessions(
    RequireAuth(auth): RequireAuth,
    State(state): State<SessionState>,
) -> Json<ApiResponse<Vec<SessionSummary>>> {
    if require_scope(&auth, &Scope::SessionRead).is_err() {
        return Json(ApiResponse::error("Forbidden: insufficient scope"));
    }
    let sessions = state.manager.list_sessions(&auth).await;
    Json(ApiResponse::success(sessions))
}

/// Create a new session.
async fn create_session(
    RequireAuth(auth): RequireAuth,
    State(state): State<SessionState>,
    Json(request): Json<CreateSessionRequest>,
) -> Json<ApiResponse<SessionSummary>> {
    if require_scope(&auth, &Scope::SessionWrite).is_err() {
        return Json(ApiResponse::error("Forbidden: insufficient scope"));
    }
    match state.manager.create_session(&auth, request.name).await {
        Ok(summary) => Json(ApiResponse::success(summary)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// Get session details.
async fn get_session(
    RequireAuth(auth): RequireAuth,
    State(state): State<SessionState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SessionSummary>>, StatusCode> {
    if require_scope(&auth, &Scope::SessionRead).is_err() {
        return Err(StatusCode::FORBIDDEN);
    }
    match state.manager.get_session(id, &auth).await {
        Ok(summary) => Ok(Json(ApiResponse::success(summary))),
        Err(e) => Ok(Json(ApiResponse::error(e.to_string()))),
    }
}

/// Delete a session.
async fn delete_session(
    RequireAuth(auth): RequireAuth,
    State(state): State<SessionState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    if require_scope(&auth, &Scope::SessionWrite).is_err() {
        return Err(StatusCode::FORBIDDEN);
    }
    match state.manager.delete_session(id, &auth).await {
        Ok(()) => Ok(Json(ApiResponse::success(()))),
        Err(e) => Ok(Json(ApiResponse::error(e.to_string()))),
    }
}

/// Send a message to a session.
async fn send_message(
    RequireAuth(auth): RequireAuth,
    State(state): State<SessionState>,
    Path(id): Path<Uuid>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<ApiResponse<SendMessageResponse>>, StatusCode> {
    if require_scope(&auth, &Scope::ExecutionWrite).is_err() {
        return Err(StatusCode::FORBIDDEN);
    }
    match state.manager.send_message(id, &request.text, &auth).await {
        Ok(Some(_)) => Ok(Json(ApiResponse::success(SendMessageResponse {
            started: true,
            queue_position: 0,
        }))),
        Ok(None) => {
            // Queued — get current queue size
            let summary = state.manager.get_session(id, &auth).await.ok();
            let pos = summary.map(|s| s.pending_count).unwrap_or(1);
            Ok(Json(ApiResponse::success(SendMessageResponse {
                started: false,
                queue_position: pos,
            })))
        }
        Err(e) => Ok(Json(ApiResponse::error(e.to_string()))),
    }
}

/// Cancel active execution for a session.
async fn cancel_execution(
    RequireAuth(auth): RequireAuth,
    State(state): State<SessionState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<bool>>, StatusCode> {
    if require_scope(&auth, &Scope::ExecutionWrite).is_err() {
        return Err(StatusCode::FORBIDDEN);
    }
    match state.manager.cancel_execution(id, &auth).await {
        Ok(was_running) => Ok(Json(ApiResponse::success(was_running))),
        Err(e) => Ok(Json(ApiResponse::error(e.to_string()))),
    }
}

/// Create sessions routes.
pub fn sessions_routes() -> Router {
    Router::new()
        .route(
            "/api/v1/sessions",
            get(list_sessions).post(create_session),
        )
        .route(
            "/api/v1/sessions/:id",
            get(get_session).delete(delete_session),
        )
        .route("/api/v1/sessions/:id/messages", post(send_message))
        .route("/api/v1/sessions/:id/cancel", post(cancel_execution))
        .with_state(SessionState::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Json;
    use cratos_core::auth::{AuthContext, AuthMethod, Scope};

    fn test_auth() -> RequireAuth {
        RequireAuth(AuthContext {
            user_id: "test".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::Admin],
            session_id: None,
            device_id: None,
        })
    }

    #[tokio::test]
    async fn test_create_and_list_sessions() {
        let state = SessionState::new();

        let response = create_session(
            test_auth(),
            State(state.clone()),
            Json(CreateSessionRequest {
                name: Some("Test Session".to_string()),
            }),
        )
        .await;
        assert!(response.0.success);

        let response = list_sessions(test_auth(), State(state)).await;
        assert!(response.0.success);
        let sessions = response.0.data.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, Some("Test Session".to_string()));
    }

    #[tokio::test]
    async fn test_send_message_starts_execution() {
        let state = SessionState::new();

        let session = create_session(
            test_auth(),
            State(state.clone()),
            Json(CreateSessionRequest { name: None }),
        )
        .await;
        let session_id = session.0.data.unwrap().id;

        let response = send_message(
            test_auth(),
            State(state),
            Path(session_id),
            Json(SendMessageRequest {
                text: "Hello".to_string(),
            }),
        )
        .await
        .unwrap();

        assert!(response.0.success);
        let data = response.0.data.unwrap();
        assert!(data.started);
        assert_eq!(data.queue_position, 0);
    }
}
