//! Error types for cratos-search

use thiserror::Error;

/// Error type for vector search operations
#[derive(Error, Debug)]
pub enum Error {
    /// Index operation failed
    #[error("Index error: {0}")]
    Index(String),

    /// Search operation failed
    #[error("Search error: {0}")]
    Search(String),

    /// ID not found in index
    #[error("ID not found: {0}")]
    NotFound(String),

    /// ID already exists
    #[error("ID already exists: {0}")]
    AlreadyExists(String),

    /// Dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimensions
        expected: usize,
        /// Actual dimensions
        actual: usize,
    },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Result type for vector search operations
pub type Result<T> = std::result::Result<T, Error>;
