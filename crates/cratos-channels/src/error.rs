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

    /// Discord error
    #[error("discord error: {0}")]
    Discord(String),

    /// WhatsApp error
    #[error("whatsapp error: {0}")]
    WhatsApp(String),

    /// Voice/audio error
    #[error("voice error: {0}")]
    Voice(String),

    /// Message parsing error
    #[error("message parsing error: {0}")]
    Parse(String),

    /// Rate limit exceeded
    #[error("rate limit exceeded")]
    RateLimit,

    /// Network error
    #[error("network error: {0}")]
    Network(String),

    /// Feature not enabled
    #[error("feature not enabled: {0}")]
    NotEnabled(String),

    /// Bridge connection error
    #[error("bridge error: {0}")]
    Bridge(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
