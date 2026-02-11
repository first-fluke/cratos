//! Cratos Audio - Voice Control
//!
//! This crate provides voice control capabilities for Cratos:
//! - Wake word detection (Silero VAD via ONNX)
//! - Speech-to-text (Whisper API via OpenAI)
//! - Text-to-speech (Edge TTS - free)
//!
//! # Features
//!
//! - `tts` (default): Enable text-to-speech (Edge TTS - free, no API key needed)
//! - `stt`: Enable speech-to-text (requires `OPENAI_API_KEY`)
//! - `full`: Enable all voice features
//!
//! # Usage
//!
//! ```rust,ignore
//! use cratos_audio::{VoiceController, VoiceConfig};
//!
//! let config = VoiceConfig::default();
//! let controller = VoiceController::new(config)?;
//!
//! // TTS only (always available)
//! let audio = controller.speak("Hello, world!").await?;
//!
//! // STT (requires OPENAI_API_KEY)
//! if controller.stt_enabled() {
//!     let text = controller.transcribe(&audio_bytes).await?;
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod error;
pub mod input;
pub mod output;
pub mod stt;
pub mod tts;
pub mod wake_word;
#[cfg(feature = "local-stt")]
pub mod whisper_local;

mod controller;

pub use config::{SttConfig, VoiceConfig, WakeWordConfig};
pub use controller::{VoiceController, VoiceEvent, VoiceMode};
pub use error::{Error, Result};
pub use stt::{SpeechToText, SttBackend};
pub use tts::TextToSpeech;
pub use wake_word::WakeWordDetector;
#[cfg(feature = "local-stt")]
pub use whisper_local::{LocalWhisper, WhisperModel};

/// Check if STT is available (API key set or local-stt feature enabled)
#[must_use]
pub fn stt_available() -> bool {
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return true;
    }
    cfg!(feature = "local-stt")
}

/// Get the default models directory
#[must_use]
pub fn default_models_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".cratos")
        .join("models")
}
