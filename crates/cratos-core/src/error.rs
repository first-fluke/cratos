//! Error types for cratos-core

use thiserror::Error;

/// Core error type
#[derive(Debug, Error)]
pub enum Error {
    /// Planning failed
    #[error("planning error: {0}")]
    Planning(String),

    /// Execution failed
    #[error("execution error: {0}")]
    Execution(String),

    /// Memory error
    #[error("memory error: {0}")]
    Memory(String),

    /// Approval timeout or rejection
    #[error("approval error: {0}")]
    Approval(String),

    /// LLM provider error
    #[error("llm error: {0}")]
    Llm(#[from] cratos_llm::Error),

    /// Tool execution error
    #[error("tool error: {0}")]
    Tool(#[from] cratos_tools::Error),

    /// Replay/logging error
    #[error("replay error: {0}")]
    Replay(#[from] cratos_replay::Error),

    /// Internal error (Redis, serialization, etc.)
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
