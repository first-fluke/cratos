//! Error types for cratos-skills

use thiserror::Error;

/// Skill system error type
#[derive(Debug, Error)]
pub enum Error {
    /// Skill not found
    #[error("skill not found: {0}")]
    SkillNotFound(String),

    /// Pattern not found
    #[error("pattern not found: {0}")]
    PatternNotFound(String),

    /// Database error
    #[error("database error: {0}")]
    Database(String),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Validation error
    #[error("validation error: {0}")]
    Validation(String),

    /// Execution error
    #[error("execution error: {0}")]
    Execution(String),

    /// Configuration error
    #[error("configuration error: {0}")]
    Configuration(String),

    /// IO error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Replay store error
    #[error("replay store error: {0}")]
    ReplayStore(#[from] cratos_replay::Error),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
