//! Error types for cratos-replay

use thiserror::Error;

/// Replay error type
#[derive(Debug, Error)]
pub enum Error {
    /// Event not found
    #[error("event not found: {0}")]
    NotFound(String),

    /// Execution not found
    #[error("execution not found: {0}")]
    ExecutionNotFound(String),

    /// Database error
    #[error("database error: {0}")]
    Database(String),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
