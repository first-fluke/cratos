//! Authentication middleware for Axum
//!
//! Extracts Bearer tokens or API keys from requests and validates them
//! against the AuthStore. Provides `RequireAuth` extractor for handlers.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use cratos_core::auth::{AuthContext, AuthError, AuthStore, Scope};
use serde::Serialize;
use std::sync::Arc;

/// JSON error response for auth failures
#[derive(Debug, Serialize)]
struct AuthErrorResponse {
    success: bool,
    error: String,
    code: String,
}

impl AuthErrorResponse {
    fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            success: false,
            error: error.into(),
            code: code.into(),
        }
    }
}

/// Auth rejection type
pub struct AuthRejection {
    status: StatusCode,
    body: AuthErrorResponse,
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

impl From<AuthError> for AuthRejection {
    fn from(err: AuthError) -> Self {
        match err {
            AuthError::MissingCredentials => AuthRejection {
                status: StatusCode::UNAUTHORIZED,
                body: AuthErrorResponse::new(
                    "Authentication required. Provide Authorization: Bearer <token> or X-API-Key header.",
                    "UNAUTHORIZED",
                ),
            },
            AuthError::InvalidCredentials => AuthRejection {
                status: StatusCode::UNAUTHORIZED,
                body: AuthErrorResponse::new("Invalid token or API key", "INVALID_CREDENTIALS"),
            },
            AuthError::TokenRevoked => AuthRejection {
                status: StatusCode::UNAUTHORIZED,
                body: AuthErrorResponse::new("Token has been revoked", "TOKEN_REVOKED"),
            },
            AuthError::InsufficientScope { required } => AuthRejection {
                status: StatusCode::FORBIDDEN,
                body: AuthErrorResponse::new(
                    format!("Insufficient permissions. Required scope: {}", required),
                    "FORBIDDEN",
                ),
            },
            AuthError::Internal(msg) => AuthRejection {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                body: AuthErrorResponse::new(msg, "INTERNAL_ERROR"),
            },
        }
    }
}

// ============================================================================
// RequireAuth Extractor
// ============================================================================

/// Axum extractor that requires authentication.
///
/// Extracts the token from:
/// 1. `Authorization: Bearer <token>` header
/// 2. `X-API-Key: <key>` header
/// 3. `?token=<token>` query parameter (for WebSocket connections)
pub struct RequireAuth(pub AuthContext);

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for RequireAuth
where
    S: Send + Sync,
{
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        // Get AuthStore from extensions
        let auth_store = parts
            .extensions
            .get::<Arc<AuthStore>>()
            .ok_or_else(|| AuthError::Internal("AuthStore not configured".to_string()))?;

        // If auth is disabled, return anonymous admin
        if !auth_store.is_enabled() {
            return Ok(RequireAuth(AuthContext {
                user_id: "anonymous".to_string(),
                method: cratos_core::auth::AuthMethod::BearerToken,
                scopes: vec![cratos_core::auth::Scope::Admin],
                session_id: None,
                device_id: None,
            }));
        }

        // Try extracting token from various sources
        let token = extract_token(parts)?;
        let ctx = auth_store.validate_token(&token)?;

        Ok(RequireAuth(ctx))
    }
}

/// Extract token from request headers or query params
fn extract_token(parts: &Parts) -> std::result::Result<String, AuthError> {
    // 1. Authorization: Bearer <token>
    if let Some(auth_header) = parts.headers.get("authorization") {
        if let Ok(value) = auth_header.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                return Ok(token.trim().to_string());
            }
        }
    }

    // 2. X-API-Key header
    if let Some(api_key_header) = parts.headers.get("x-api-key") {
        if let Ok(value) = api_key_header.to_str() {
            return Ok(value.trim().to_string());
        }
    }

    // 3. ?token= query parameter (for WebSocket upgrades)
    if let Some(query) = parts.uri.query() {
        for param in query.split('&') {
            if let Some(token) = param.strip_prefix("token=") {
                return Ok(token.to_string());
            }
        }
    }

    Err(AuthError::MissingCredentials)
}

// ============================================================================
// RequireAuthStrict Extractor
// ============================================================================

/// Axum extractor that **always** requires a valid token, even when global
/// authentication is disabled. Use this for sensitive endpoints such as
/// WebSocket chat/events and `/health/detailed` which expose infrastructure
/// information that must never be publicly accessible.
pub struct RequireAuthStrict(pub AuthContext);

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for RequireAuthStrict
where
    S: Send + Sync,
{
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let auth_store = parts
            .extensions
            .get::<Arc<AuthStore>>()
            .ok_or_else(|| AuthError::Internal("AuthStore not configured".to_string()))?;

        // NEVER bypass — always require a valid token regardless of is_enabled()
        let token = extract_token(parts)?;
        let ctx = auth_store.validate_token(&token)?;
        Ok(RequireAuthStrict(ctx))
    }
}

// ============================================================================
// RequireScope helper
// ============================================================================

/// Check scope requirement. Use RequireAuth then check scopes in handler.
pub fn require_scope(auth: &AuthContext, scope: &Scope) -> std::result::Result<(), AuthRejection> {
    auth.require_scope(scope).map_err(AuthRejection::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_error_response_unauthorized() {
        let rejection = AuthRejection::from(AuthError::MissingCredentials);
        assert_eq!(rejection.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_require_auth_strict_type_exists() {
        // Verify RequireAuthStrict struct exists and wraps AuthContext
        let ctx = AuthContext {
            user_id: "test".to_string(),
            method: cratos_core::auth::AuthMethod::ApiKey,
            scopes: vec![Scope::Admin],
            session_id: None,
            device_id: None,
        };
        let strict = RequireAuthStrict(ctx);
        assert_eq!(strict.0.user_id, "test");
        assert!(strict.0.has_scope(&Scope::Admin));
    }

    #[test]
    fn test_auth_error_response_forbidden() {
        let rejection = AuthRejection::from(AuthError::InsufficientScope {
            required: "config_write".to_string(),
        });
        assert_eq!(rejection.status, StatusCode::FORBIDDEN);
    }
}
