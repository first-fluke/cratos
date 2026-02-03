//! Error types for cratos-canvas
//!
//! This module provides error types for the canvas system,
//! including WebSocket, storage, and rendering errors.

use thiserror::Error;
use uuid::Uuid;

/// Canvas error type
#[derive(Debug, Error)]
pub enum Error {
    /// Session not found
    #[error("session not found: {0}")]
    SessionNotFound(Uuid),

    /// Block not found
    #[error("block not found: {0}")]
    BlockNotFound(Uuid),

    /// Document not found
    #[error("document not found: {0}")]
    DocumentNotFound(Uuid),

    /// WebSocket error
    #[error("websocket error: {0}")]
    WebSocket(String),

    /// Connection closed
    #[error("connection closed")]
    ConnectionClosed,

    /// Invalid message format
    #[error("invalid message: {0}")]
    InvalidMessage(String),

    /// Database error
    #[error("database error: {0}")]
    Database(String),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Rendering error
    #[error("rendering error: {0}")]
    Rendering(String),

    /// AI error
    #[error("AI error: {0}")]
    Ai(String),

    /// Execution error
    #[error("execution error: {0}")]
    Execution(String),

    /// Rate limited
    #[error("rate limited: retry after {retry_after_secs} seconds")]
    RateLimited {
        /// Seconds to wait before retrying
        retry_after_secs: u64,
    },

    /// Permission denied
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Session expired
    #[error("session expired: {0}")]
    SessionExpired(Uuid),

    /// Maximum sessions exceeded
    #[error("maximum sessions exceeded for user: {0}")]
    MaxSessionsExceeded(String),

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Create a WebSocket error
    #[must_use]
    pub fn websocket(msg: impl Into<String>) -> Self {
        Self::WebSocket(msg.into())
    }

    /// Create a database error
    #[must_use]
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Create a serialization error
    #[must_use]
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }

    /// Create an invalid message error
    #[must_use]
    pub fn invalid_message(msg: impl Into<String>) -> Self {
        Self::InvalidMessage(msg.into())
    }

    /// Create an AI error
    #[must_use]
    pub fn ai(msg: impl Into<String>) -> Self {
        Self::Ai(msg.into())
    }

    /// Check if error is recoverable
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::ConnectionClosed | Self::WebSocket(_)
        )
    }

    /// Get error code for protocol messages
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::SessionNotFound(_) => "session_not_found",
            Self::BlockNotFound(_) => "block_not_found",
            Self::DocumentNotFound(_) => "document_not_found",
            Self::WebSocket(_) => "websocket_error",
            Self::ConnectionClosed => "connection_closed",
            Self::InvalidMessage(_) => "invalid_message",
            Self::Database(_) => "database_error",
            Self::Serialization(_) => "serialization_error",
            Self::Rendering(_) => "rendering_error",
            Self::Ai(_) => "ai_error",
            Self::Execution(_) => "execution_error",
            Self::RateLimited { .. } => "rate_limited",
            Self::PermissionDenied(_) => "permission_denied",
            Self::SessionExpired(_) => "session_expired",
            Self::MaxSessionsExceeded(_) => "max_sessions_exceeded",
            Self::Internal(_) => "internal_error",
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err.to_string())
    }
}

impl From<axum::Error> for Error {
    fn from(err: axum::Error) -> Self {
        Self::WebSocket(err.to_string())
    }
}

/// Result type alias for canvas operations
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        let err = Error::SessionNotFound(Uuid::nil());
        assert_eq!(err.code(), "session_not_found");

        let err = Error::RateLimited {
            retry_after_secs: 30,
        };
        assert_eq!(err.code(), "rate_limited");
    }

    #[test]
    fn test_error_is_recoverable() {
        assert!(Error::RateLimited {
            retry_after_secs: 30
        }
        .is_recoverable());
        assert!(Error::ConnectionClosed.is_recoverable());
        assert!(!Error::SessionNotFound(Uuid::nil()).is_recoverable());
    }

    #[test]
    fn test_error_constructors() {
        let err = Error::websocket("connection failed");
        assert_eq!(err.code(), "websocket_error");

        let err = Error::database("query failed");
        assert_eq!(err.code(), "database_error");
    }

    #[test]
    fn test_error_display() {
        let err = Error::SessionNotFound(Uuid::nil());
        let msg = err.to_string();
        assert!(msg.contains("session not found"));
    }

    #[test]
    fn test_from_serde_error() {
        let result: std::result::Result<i32, serde_json::Error> =
            serde_json::from_str("not valid json");
        let err: Error = result.unwrap_err().into();
        assert_eq!(err.code(), "serialization_error");
    }
}
