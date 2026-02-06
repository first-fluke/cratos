//! Speech-to-Text engine with multiple backends
//!
//! Supports:
//! - OpenAI Whisper API (requires `OPENAI_API_KEY`)
//! - Local Whisper via candle (requires `local-stt` feature)
//! - Auto mode: local first, API fallback

use crate::config::SttConfig;
use crate::error::{Error, Result};
use async_openai::{
    config::OpenAIConfig,
    types::audio::{AudioInput, AudioResponseFormat, CreateTranscriptionRequestArgs},
    Client,
};
use tracing::{debug, info, warn};

/// STT backend selection
#[derive(Debug, Clone)]
pub enum SttBackend {
    /// OpenAI Whisper API only
    Api,
    /// Local Whisper model only (requires `local-stt` feature)
    #[cfg(feature = "local-stt")]
    Local(crate::whisper_local::WhisperModel),
    /// Auto: try local first, fall back to API
    Auto,
}

impl SttBackend {
    /// Parse from config strings
    pub fn from_config(config: &SttConfig) -> Self {
        match config.backend.as_str() {
            "api" => Self::Api,
            #[cfg(feature = "local-stt")]
            "local" => Self::Local(crate::whisper_local::WhisperModel::parse(&config.model)),
            #[cfg(not(feature = "local-stt"))]
            "local" => {
                warn!("local-stt feature not enabled, falling back to API");
                Self::Api
            }
            _ => Self::Auto, // "auto" or anything else
        }
    }
}

/// Speech-to-text engine using Whisper API
pub struct SpeechToText {
    client: Option<Client<OpenAIConfig>>,
    language: String,
    api_enabled: bool,
    backend: SttBackend,
    #[cfg(feature = "local-stt")]
    local_whisper: Option<crate::whisper_local::LocalWhisper>,
}

impl SpeechToText {
    /// Create a new STT engine
    ///
    /// If `OPENAI_API_KEY` is not set, the API backend is disabled.
    #[must_use]
    pub fn new(language: &str) -> Self {
        Self::with_config(language, &SttConfig::default())
    }

    /// Create with explicit config
    #[must_use]
    pub fn with_config(language: &str, config: &SttConfig) -> Self {
        let api_enabled = std::env::var("OPENAI_API_KEY").is_ok();

        let client = if api_enabled {
            Some(Client::new())
        } else {
            warn!("OPENAI_API_KEY not set - API STT disabled");
            None
        };

        let backend = SttBackend::from_config(config);

        #[cfg(feature = "local-stt")]
        let local_whisper = match &backend {
            SttBackend::Local(model) => {
                Some(crate::whisper_local::LocalWhisper::new(*model, language))
            }
            SttBackend::Auto => {
                let model = crate::whisper_local::WhisperModel::parse(&config.model);
                Some(crate::whisper_local::LocalWhisper::new(model, language))
            }
            SttBackend::Api => None,
        };

        let is_enabled = api_enabled || cfg!(feature = "local-stt");
        if is_enabled {
            info!(
                "STT initialized (language: {}, backend: {:?})",
                language, backend
            );
        } else {
            warn!("No STT backend available. Set OPENAI_API_KEY or enable local-stt feature.");
        }

        Self {
            client,
            language: language.to_string(),
            api_enabled,
            backend,
            #[cfg(feature = "local-stt")]
            local_whisper,
        }
    }

    /// Check if any STT backend is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        if self.api_enabled {
            return true;
        }
        #[cfg(feature = "local-stt")]
        if self.local_whisper.is_some() {
            return true;
        }
        false
    }

    /// Get the configured language
    #[must_use]
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Transcribe audio to text
    ///
    /// Routes to the appropriate backend based on configuration.
    pub async fn transcribe(&self, audio_bytes: &[u8]) -> Result<String> {
        // Validate audio data (basic WAV check)
        if audio_bytes.len() < 44 {
            return Err(Error::Stt("Audio data too short".to_string()));
        }

        if &audio_bytes[0..4] != b"RIFF" || &audio_bytes[8..12] != b"WAVE" {
            return Err(Error::Stt("Invalid WAV format".to_string()));
        }

        match &self.backend {
            SttBackend::Api => self.transcribe_api(audio_bytes).await,
            #[cfg(feature = "local-stt")]
            SttBackend::Local(_) => self.transcribe_local(audio_bytes).await,
            SttBackend::Auto => self.transcribe_auto(audio_bytes).await,
        }
    }

    /// Transcribe via OpenAI Whisper API
    async fn transcribe_api(&self, audio_bytes: &[u8]) -> Result<String> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| Error::NotEnabled("STT API requires OPENAI_API_KEY".to_string()))?;

        debug!("Transcribing {} bytes via API", audio_bytes.len());

        let request = CreateTranscriptionRequestArgs::default()
            .file(AudioInput::from_vec_u8(
                "audio.wav".to_string(),
                audio_bytes.to_vec(),
            ))
            .model("whisper-1")
            .language(&self.language)
            .response_format(AudioResponseFormat::Text)
            .build()
            .map_err(|e| Error::Stt(format!("Failed to build request: {e}")))?;

        let response = client
            .audio()
            .transcription()
            .create(request)
            .await
            .map_err(|e| Error::Stt(format!("Transcription failed: {e}")))?;

        let text = response.text.trim().to_string();
        debug!("API transcription result: {}", text);

        Ok(text)
    }

    /// Transcribe via local Whisper model
    #[cfg(feature = "local-stt")]
    async fn transcribe_local(&self, audio_bytes: &[u8]) -> Result<String> {
        debug!("Transcribing {} bytes via local Whisper", audio_bytes.len());

        // Clone necessary data for the blocking task
        let bytes = audio_bytes.to_vec();
        let model_size = match &self.backend {
            SttBackend::Local(m) => *m,
            _ => crate::whisper_local::WhisperModel::parse("small"),
        };
        let language = self.language.clone();

        // Run inference in a blocking thread
        let text = tokio::task::spawn_blocking(move || {
            let mut whisper = crate::whisper_local::LocalWhisper::new(model_size, &language);
            whisper.transcribe_sync(&bytes)
        })
        .await
        .map_err(|e| Error::Stt(format!("Blocking task failed: {e}")))??;

        debug!("Local transcription result: {}", text);
        Ok(text)
    }

    /// Auto mode: try local first, fall back to API
    async fn transcribe_auto(&self, audio_bytes: &[u8]) -> Result<String> {
        // Try local first if available
        #[cfg(feature = "local-stt")]
        if self.local_whisper.is_some() {
            match self.transcribe_local(audio_bytes).await {
                Ok(text) => return Ok(text),
                Err(e) => {
                    warn!("Local STT failed, trying API: {}", e);
                }
            }
        }

        // Fall back to API
        if self.api_enabled {
            return self.transcribe_api(audio_bytes).await;
        }

        Err(Error::NotEnabled(
            "No STT backend available. Set OPENAI_API_KEY or enable local-stt feature.".to_string(),
        ))
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
            .map_err(|e| Error::Stt(format!("Failed to build request: {e}")))?;

        let response = client
            .audio()
            .transcription()
            .create(request)
            .await
            .map_err(|e| Error::Stt(format!("Transcription failed: {e}")))?;

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

    /// Estimate cost for API transcription (USD)
    ///
    /// Whisper API costs $0.006 per minute.
    /// Local transcription is free.
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
        let saved_key = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("OPENAI_API_KEY");

        let _stt = SpeechToText::new("ko");

        // Without local-stt feature and without API key, STT should be disabled
        #[cfg(not(feature = "local-stt"))]
        assert!(!_stt.is_enabled());

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

    #[test]
    fn test_backend_from_config() {
        let config = SttConfig {
            backend: "api".to_string(),
            model: "small".to_string(),
        };
        assert!(matches!(SttBackend::from_config(&config), SttBackend::Api));

        let config = SttConfig {
            backend: "auto".to_string(),
            model: "small".to_string(),
        };
        assert!(matches!(SttBackend::from_config(&config), SttBackend::Auto));
    }
}
