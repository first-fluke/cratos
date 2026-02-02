//! Error types for cratos-channels

use thiserror::Error;

/// Channel error type
#[derive(Debug, Error)]
pub enum Error {
    /// Telegram error
    #[error("telegram error: {0}")]
    Telegram(String),

    /// Slack error
    #[error("slack error: {0}")]
    Slack(String),

    /// Message parsing error
    #[error("message parsing error: {0}")]
    Parse(String),

    /// Rate limit exceeded
    #[error("rate limit exceeded")]
    RateLimit,

    /// Network error
    #[error("network error: {0}")]
    Network(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
