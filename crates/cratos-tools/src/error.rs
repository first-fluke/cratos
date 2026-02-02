//! Error types for cratos-tools

use thiserror::Error;

/// Tool error type
#[derive(Debug, Error)]
pub enum Error {
    /// Tool not found
    #[error("tool not found: {0}")]
    NotFound(String),

    /// Tool execution failed
    #[error("execution failed: {0}")]
    Execution(String),

    /// Invalid input
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Permission denied
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Timeout
    #[error("timeout after {0}ms")]
    Timeout(u64),

    /// Network error
    #[error("network error: {0}")]
    Network(String),

    /// IO error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
