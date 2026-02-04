//! Speech-to-Text using OpenAI Whisper API
//!
//! Requires `OPENAI_API_KEY` environment variable to be set.
//! If not set, STT functionality is disabled.

use crate::error::{Error, Result};
use async_openai::{
    config::OpenAIConfig,
    types::audio::{AudioInput, AudioResponseFormat, CreateTranscriptionRequestArgs},
    Client,
};
use tracing::{debug, info, warn};

/// Speech-to-text engine using Whisper API
pub struct SpeechToText {
    client: Option<Client<OpenAIConfig>>,
    language: String,
    enabled: bool,
}

impl SpeechToText {
    /// Create a new STT engine
    ///
    /// If `OPENAI_API_KEY` is not set, the engine is created but disabled.
    #[must_use]
    pub fn new(language: &str) -> Self {
        let enabled = std::env::var("OPENAI_API_KEY").is_ok();

        let client = if enabled {
            Some(Client::new())
        } else {
            warn!("OPENAI_API_KEY not set - STT disabled");
            warn!("To enable voice recognition, set: export OPENAI_API_KEY=\"sk-...\"");
            None
        };

        if enabled {
            info!("Whisper STT initialized (language: {})", language);
        }

        Self {
            client,
            language: language.to_string(),
            enabled,
        }
    }

    /// Check if STT is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the configured language
    #[must_use]
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Transcribe audio to text
    ///
    /// # Arguments
    /// * `audio_bytes` - WAV audio data
    ///
    /// # Returns
    /// Transcribed text, or error if STT is disabled
    pub async fn transcribe(&self, audio_bytes: &[u8]) -> Result<String> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| Error::NotEnabled("STT requires OPENAI_API_KEY".to_string()))?;

        // Validate audio data (basic WAV check)
        if audio_bytes.len() < 44 {
            return Err(Error::Stt("Audio data too short".to_string()));
        }

        if &audio_bytes[0..4] != b"RIFF" || &audio_bytes[8..12] != b"WAVE" {
            return Err(Error::Stt("Invalid WAV format".to_string()));
        }

        debug!("Transcribing {} bytes of audio", audio_bytes.len());

        let request = CreateTranscriptionRequestArgs::default()
            .file(AudioInput::from_vec_u8(
                "audio.wav".to_string(),
                audio_bytes.to_vec(),
            ))
            .model("whisper-1")
            .language(&self.language)
            .response_format(AudioResponseFormat::Text)
            .build()
            .map_err(|e| Error::Stt(format!("Failed to build request: {}", e)))?;

        let response = client
            .audio()
            .transcription()
            .create(request)
            .await
            .map_err(|e| Error::Stt(format!("Transcription failed: {}", e)))?;

        let text = response.text.trim().to_string();
        debug!("Transcription result: {}", text);

        Ok(text)
    }

    /// Transcribe with custom prompt for better context
    pub async fn transcribe_with_prompt(&self, audio_bytes: &[u8], prompt: &str) -> Result<String> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| Error::NotEnabled("STT requires OPENAI_API_KEY".to_string()))?;

        let request = CreateTranscriptionRequestArgs::default()
            .file(AudioInput::from_vec_u8(
                "audio.wav".to_string(),
                audio_bytes.to_vec(),
            ))
            .model("whisper-1")
            .language(&self.language)
            .prompt(prompt)
            .response_format(AudioResponseFormat::Text)
            .build()
            .map_err(|e| Error::Stt(format!("Failed to build request: {}", e)))?;

        let response = client
            .audio()
            .transcription()
            .create(request)
            .await
            .map_err(|e| Error::Stt(format!("Transcription failed: {}", e)))?;

        Ok(response.text.trim().to_string())
    }

    /// Get supported languages
    #[must_use]
    pub fn supported_languages() -> &'static [(&'static str, &'static str)] {
        &[
            ("ko", "Korean"),
            ("en", "English"),
            ("ja", "Japanese"),
            ("zh", "Chinese"),
            ("es", "Spanish"),
            ("fr", "French"),
            ("de", "German"),
            ("it", "Italian"),
            ("pt", "Portuguese"),
            ("ru", "Russian"),
        ]
    }

    /// Estimate cost for transcription (USD)
    ///
    /// Whisper API costs $0.006 per minute
    #[must_use]
    pub fn estimate_cost(audio_duration_secs: f64) -> f64 {
        let minutes = audio_duration_secs / 60.0;
        minutes * 0.006
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stt_disabled_without_api_key() {
        // This test assumes OPENAI_API_KEY is not set in test environment
        // If it is set, this test will fail
        let saved_key = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("OPENAI_API_KEY");

        let stt = SpeechToText::new("ko");
        assert!(!stt.is_enabled());

        // Restore key if it was set
        if let Some(key) = saved_key {
            std::env::set_var("OPENAI_API_KEY", key);
        }
    }

    #[test]
    fn test_cost_estimation() {
        // 1 minute = $0.006
        assert!((SpeechToText::estimate_cost(60.0) - 0.006).abs() < 0.0001);

        // 5 minutes = $0.03
        assert!((SpeechToText::estimate_cost(300.0) - 0.03).abs() < 0.0001);
    }

    #[test]
    fn test_supported_languages() {
        let languages = SpeechToText::supported_languages();
        assert!(!languages.is_empty());
        assert!(languages.iter().any(|(code, _)| *code == "ko"));
        assert!(languages.iter().any(|(code, _)| *code == "en"));
    }
}
