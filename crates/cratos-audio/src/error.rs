//! Error types for cratos-audio

use thiserror::Error;

/// Audio/voice error type
#[derive(Debug, Error)]
pub enum Error {
    /// Audio device error
    #[error("audio device error: {0}")]
    AudioDevice(String),

    /// Audio stream error
    #[error("audio stream error: {0}")]
    AudioStream(String),

    /// Wake word detection error
    #[error("wake word error: {0}")]
    WakeWord(String),

    /// Speech-to-text error
    #[error("STT error: {0}")]
    Stt(String),

    /// Text-to-speech error
    #[error("TTS error: {0}")]
    Tts(String),

    /// Feature not enabled
    #[error("feature not enabled: {0}")]
    NotEnabled(String),

    /// Model not found
    #[error("model not found: {0}")]
    ModelNotFound(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Network error
    #[error("network error: {0}")]
    Network(String),

    /// Configuration error
    #[error("configuration error: {0}")]
    Config(String),

    /// ONNX runtime error
    #[error("ONNX error: {0}")]
    Onnx(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
