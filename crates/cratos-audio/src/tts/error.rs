use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Rate limit exceeded: {limit_type} (retry after {retry_after:?})")]
    RateLimitExceeded {
        limit_type: String,
        retry_after: Option<Duration>,
    },

    #[error("Text too long: {0} chars (max: {1})")]
    TextTooLong(usize, usize),

    #[error("Empty text")]
    EmptyText,

    #[error("Invalid voice ID")]
    InvalidVoiceId,

    #[error("API key not found")]
    ApiKeyNotFound,

    #[error("All backends failed")]
    AllBackendsFailed,

    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid header value")]
    InvalidHeaderValue(#[from] reqwest::header::ToStrError),

    #[error("Parse integer error")]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error("Credentials error: {0}")]
    CredentialsError(String),
    #[error("Edge TTS error: {0}")]
    EdgeError(String),
}

pub type Result<T> = std::result::Result<T, TtsError>;
