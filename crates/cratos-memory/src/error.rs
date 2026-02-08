//! Error types for the graph memory system.

/// Errors that can occur in graph memory operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// SQLite database error
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Migration error
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// Serialization / deserialization error
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Entity extraction error
    #[error("extraction error: {0}")]
    Extraction(String),

    /// Embedding error
    #[error("embedding error: {0}")]
    Embedding(String),

    /// General internal error
    #[error("{0}")]
    Internal(String),
}

/// Convenience Result type.
pub type Result<T> = std::result::Result<T, Error>;
