//! Voice configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Speech-to-text configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttConfig {
    /// Backend: "auto" | "api" | "local"
    /// - auto: local first, API fallback
    /// - api: OpenAI Whisper API only
    /// - local: local candle Whisper only
    #[serde(default = "default_stt_backend")]
    pub backend: String,

    /// Local model size: "tiny" | "base" | "small"
    /// Korean requires "small" for reasonable accuracy
    #[serde(default = "default_stt_model")]
    pub model: String,
}

fn default_stt_backend() -> String {
    "auto".to_string()
}

fn default_stt_model() -> String {
    "small".to_string()
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            backend: default_stt_backend(),
            model: default_stt_model(),
        }
    }
}

/// Voice controller configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    /// Wake word configuration
    #[serde(default)]
    pub wake_word: WakeWordConfig,

    /// Language for STT and TTS
    #[serde(default = "default_language")]
    pub language: String,

    /// Voice detection threshold (0.0 - 1.0)
    #[serde(default = "default_threshold")]
    pub threshold: f32,

    /// TTS voice name (platform-specific)
    #[serde(default)]
    pub tts_voice: Option<String>,

    /// STT configuration
    #[serde(default)]
    pub stt: SttConfig,

    /// Models directory
    #[serde(default = "default_models_dir")]
    pub models_dir: PathBuf,

    /// Sample rate for audio capture
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,

    /// Silence duration (ms) to stop listening
    #[serde(default = "default_silence_duration")]
    pub silence_duration_ms: u64,

    /// Maximum recording duration (seconds)
    #[serde(default = "default_max_duration")]
    pub max_duration_secs: u64,
}

fn default_language() -> String {
    "ko".to_string()
}

fn default_threshold() -> f32 {
    0.5
}

fn default_models_dir() -> PathBuf {
    crate::default_models_dir()
}

fn default_sample_rate() -> u32 {
    16000
}

fn default_silence_duration() -> u64 {
    1500 // 1.5 seconds
}

fn default_max_duration() -> u64 {
    30 // 30 seconds max recording
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            wake_word: WakeWordConfig::default(),
            language: default_language(),
            threshold: default_threshold(),
            tts_voice: None,
            stt: SttConfig::default(),
            models_dir: default_models_dir(),
            sample_rate: default_sample_rate(),
            silence_duration_ms: default_silence_duration(),
            max_duration_secs: default_max_duration(),
        }
    }
}

impl VoiceConfig {
    /// Create with a custom wake word name
    #[must_use]
    pub fn with_wake_word_name(mut self, name: impl Into<String>) -> Self {
        self.wake_word.name = name.into();
        self
    }

    /// Set language
    #[must_use]
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }

    /// Set TTS voice
    #[must_use]
    pub fn with_tts_voice(mut self, voice: impl Into<String>) -> Self {
        self.tts_voice = Some(voice.into());
        self
    }

    /// Set models directory
    #[must_use]
    pub fn with_models_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.models_dir = path.into();
        self
    }

    /// Get the wake word model path
    #[must_use]
    pub fn wake_word_model_path(&self) -> PathBuf {
        self.models_dir.join("silero_vad.onnx")
    }

    /// Get TTS voice name based on language
    #[must_use]
    pub fn get_tts_voice(&self) -> &str {
        if let Some(ref voice) = self.tts_voice {
            return voice;
        }

        match self.language.as_str() {
            "ko" | "korean" => "ko-KR-SunHiNeural",
            "en" | "english" => "en-US-JennyNeural",
            "ja" | "japanese" => "ja-JP-NanamiNeural",
            "zh" | "chinese" => "zh-CN-XiaoxiaoNeural",
            _ => "en-US-JennyNeural",
        }
    }
}

/// Wake word configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeWordConfig {
    /// Primary wake word name
    #[serde(default = "default_wake_word_name")]
    pub name: String,

    /// Alternative wake word patterns
    #[serde(default)]
    pub alternatives: Vec<String>,

    /// Detection sensitivity (0.0 - 1.0)
    #[serde(default = "default_threshold")]
    pub sensitivity: f32,
}

fn default_wake_word_name() -> String {
    "크레토스".to_string()
}

impl Default for WakeWordConfig {
    fn default() -> Self {
        Self {
            name: default_wake_word_name(),
            alternatives: vec!["크라토스".to_string(), "Hey Cratos".to_string()],
            sensitivity: default_threshold(),
        }
    }
}

impl WakeWordConfig {
    /// Create with a custom name
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            alternatives: Vec::new(),
            sensitivity: default_threshold(),
        }
    }

    /// Add alternative patterns
    #[must_use]
    pub fn with_alternatives(mut self, alternatives: Vec<String>) -> Self {
        self.alternatives = alternatives;
        self
    }

    /// Get all wake word patterns (name + alternatives)
    #[must_use]
    pub fn all_patterns(&self) -> Vec<&str> {
        let mut patterns = vec![self.name.as_str()];
        patterns.extend(self.alternatives.iter().map(|s| s.as_str()));
        patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = VoiceConfig::default();
        assert_eq!(config.language, "ko");
        assert_eq!(config.wake_word.name, "크레토스");
        assert!(!config.wake_word.alternatives.is_empty());
    }

    #[test]
    fn test_tts_voice_selection() {
        let config = VoiceConfig::default().with_language("ko");
        assert!(config.get_tts_voice().contains("ko-KR"));

        let config = VoiceConfig::default().with_language("en");
        assert!(config.get_tts_voice().contains("en-US"));
    }

    #[test]
    fn test_custom_wake_word() {
        let config = VoiceConfig::default().with_wake_word_name("자비스");
        assert_eq!(config.wake_word.name, "자비스");
    }

    #[test]
    fn test_all_patterns() {
        let wake_word = WakeWordConfig::new("크레토스")
            .with_alternatives(vec!["Jarvis".to_string(), "자비스".to_string()]);
        let patterns = wake_word.all_patterns();
        assert_eq!(patterns.len(), 3);
        assert!(patterns.contains(&"크레토스"));
        assert!(patterns.contains(&"Jarvis"));
    }
}
