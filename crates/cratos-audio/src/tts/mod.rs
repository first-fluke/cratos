pub mod backend;
pub mod config;
pub mod edge;
pub mod elevenlabs;
pub mod error;
pub mod manager;
pub mod secret;
mod rate_limit;

pub use backend::{TtsBackend, TtsOptions, VoiceInfo};
pub use config::ElevenLabsConfig;
pub use edge::TextToSpeech as EdgeTts;  // Re-export old struct name for compatibility if needed
pub use elevenlabs::ElevenLabsBackend;
pub use error::{Result, TtsError};
pub use manager::TtsManager;

// Re-export Edge TTS logic (legacy support)
// In a real refactor, we would wrap Edge TTS into TtsBackend trait too.
// For now, we expose the old struct from edge module.
pub use edge::TextToSpeech;
