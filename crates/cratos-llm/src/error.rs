//! Error types for cratos-llm

use thiserror::Error;

/// LLM error type
#[derive(Debug, Error)]
pub enum Error {
    /// Provider not configured
    #[error("provider not configured: {0}")]
    NotConfigured(String),

    /// API error
    #[error("api error: {0}")]
    Api(String),

    /// Rate limit exceeded
    #[error("rate limit exceeded")]
    RateLimit,

    /// Invalid response
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// Schema validation failed
    #[error("schema validation failed: {0}")]
    SchemaValidation(String),

    /// Network error
    #[error("network error: {0}")]
    Network(String),

    /// Timeout
    #[error("timeout after {0}ms")]
    Timeout(u64),

    /// Embedding provider error
    #[error("provider error: {0}")]
    Provider(String),

    /// OAuth error
    #[error("oauth error: {0}")]
    OAuth(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
