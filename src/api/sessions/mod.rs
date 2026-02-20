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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::config::ApiResponse;
use crate::middleware::auth::{require_scope, RequireAuth};
use cratos_core::auth::Scope;

use cratos_crypto::{generate_keypair, EncryptedData, SessionCipher};

/// Shared session manager state.
#[derive(Clone)]
pub struct SessionState {
    manager: Arc<SessionManager>,
    /// E2E session ciphers keyed by session_id
    e2e_ciphers: Arc<RwLock<HashMap<Uuid, Arc<SessionCipher>>>>,
}

impl SessionState {
    /// Create a new session state.
    pub fn new() -> Self {
        Self {
            manager: Arc::new(SessionManager::new()),
            e2e_ciphers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the shared E2E cipher map (for WebSocket integration).
    pub fn e2e_ciphers(&self) -> Arc<RwLock<HashMap<Uuid, Arc<SessionCipher>>>> {
        self.e2e_ciphers.clone()
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

/// Request to initialize E2E encrypted session.
#[derive(Debug, Deserialize)]
pub struct InitE2eRequest {
    /// Client's X25519 public key (base64-encoded, 32 bytes)
    pub client_public_key: String,
}

/// Response from E2E session initialization.
#[derive(Debug, Serialize)]
pub struct InitE2eResponse {
    /// Session ID for this E2E session
    pub session_id: Uuid,
    /// Server's X25519 public key (base64-encoded, 32 bytes)
    pub server_public_key: String,
}

/// Request to send an encrypted message.
#[derive(Debug, Deserialize)]
pub struct EncryptedMessageRequest {
    /// E2E session ID
    pub session_id: Uuid,
    /// Encrypted data
    pub encrypted: EncryptedData,
}

/// Response with encrypted data.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct EncryptedMessageResponse {
    /// Encrypted response data
    pub encrypted: EncryptedData,
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

/// Initialize an E2E encrypted session via X25519 key exchange.
async fn init_e2e(
    RequireAuth(_auth): RequireAuth,
    State(state): State<SessionState>,
    Json(request): Json<InitE2eRequest>,
) -> Json<ApiResponse<InitE2eResponse>> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;

    // Decode client public key
    let client_pub_bytes = match b64.decode(&request.client_public_key) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Json(ApiResponse::error(format!(
                "Invalid base64 public key: {}",
                e
            )));
        }
    };

    if client_pub_bytes.len() != 32 {
        return Json(ApiResponse::error(
            "Public key must be exactly 32 bytes".to_string(),
        ));
    }

    let mut client_pub_arr = [0u8; 32];
    client_pub_arr.copy_from_slice(&client_pub_bytes);
    let client_public = x25519_dalek::PublicKey::from(client_pub_arr);

    // Generate server keypair
    let (server_secret, server_public) = generate_keypair();

    // Derive shared session cipher
    let cipher = SessionCipher::from_key_exchange(&server_secret, &client_public);

    // Store cipher
    let session_id = Uuid::new_v4();
    {
        let mut ciphers = state.e2e_ciphers.write().await;
        ciphers.insert(session_id, Arc::new(cipher));
    }

    let server_pub_b64 = b64.encode(server_public.as_bytes());

    Json(ApiResponse::success(InitE2eResponse {
        session_id,
        server_public_key: server_pub_b64,
    }))
}

/// Decrypt an incoming encrypted message using the E2E session cipher.
async fn decrypt_message(
    RequireAuth(_auth): RequireAuth,
    State(state): State<SessionState>,
    Json(request): Json<EncryptedMessageRequest>,
) -> Json<ApiResponse<String>> {
    let ciphers = state.e2e_ciphers.read().await;
    let Some(cipher) = ciphers.get(&request.session_id) else {
        return Json(ApiResponse::error("E2E session not found"));
    };

    match cipher.decrypt(&request.encrypted) {
        Ok(plaintext) => match String::from_utf8(plaintext) {
            Ok(text) => Json(ApiResponse::success(text)),
            Err(_) => Json(ApiResponse::error("Decrypted data is not valid UTF-8")),
        },
        Err(e) => Json(ApiResponse::error(format!("Decryption failed: {}", e))),
    }
}

/// Create sessions routes with a shared SessionState.
pub fn sessions_routes_with_state(state: SessionState) -> Router {
    Router::new()
        .route("/api/v1/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/v1/sessions/:id",
            get(get_session).delete(delete_session),
        )
        .route("/api/v1/sessions/:id/messages", post(send_message))
        .route("/api/v1/sessions/:id/cancel", post(cancel_execution))
        .route("/api/v1/sessions/init-e2e", post(init_e2e))
        .route("/api/v1/sessions/decrypt", post(decrypt_message))
        .with_state(state)
}

#[cfg(test)]
mod tests;
