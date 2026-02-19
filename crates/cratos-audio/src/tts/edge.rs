//! Text-to-Speech using Edge TTS (Microsoft)
//!
//! Edge TTS is a **free** Microsoft text-to-speech service.
//! No API key required!

use super::backend::{TtsBackend, TtsOptions, VoiceInfo};
use crate::error::{Error, Result};
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::Client;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Edge TTS endpoint
const EDGE_TTS_ENDPOINT: &str =
    "https://speech.platform.bing.com/consumer/speech/synthesize/readaloud/voice/v1";

/// Edge TTS trusted client token (public)
const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";

/// Text-to-speech engine using Edge TTS
pub struct TextToSpeech {
    client: Client,
    voice: String,
}

impl TextToSpeech {
    /// Create a new TTS engine
    #[must_use]
    pub fn new(language: &str) -> Self {
        let voice = Self::get_voice_for_language(language).to_string();

        info!("Edge TTS initialized (voice: {})", voice);

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            voice,
        }
    }

    /// Create with a specific voice
    #[must_use]
    pub fn with_voice(voice: impl Into<String>) -> Self {
        let voice = voice.into();
        info!("Edge TTS initialized (voice: {})", voice);

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            voice,
        }
    }

    /// Get the best voice for a language
    #[must_use]
    pub fn get_voice_for_language(language: &str) -> &'static str {
        match language.to_lowercase().as_str() {
            "ko" | "korean" | "ko-kr" => "ko-KR-SunHiNeural",
            "en" | "english" | "en-us" => "en-US-JennyNeural",
            "en-gb" => "en-GB-SoniaNeural",
            "ja" | "japanese" | "ja-jp" => "ja-JP-NanamiNeural",
            "zh" | "chinese" | "zh-cn" => "zh-CN-XiaoxiaoNeural",
            "zh-tw" => "zh-TW-HsiaoChenNeural",
            "es" | "spanish" | "es-es" => "es-ES-ElviraNeural",
            "fr" | "french" | "fr-fr" => "fr-FR-DeniseNeural",
            "de" | "german" | "de-de" => "de-DE-KatjaNeural",
            "it" | "italian" | "it-it" => "it-IT-ElsaNeural",
            "pt" | "portuguese" | "pt-br" => "pt-BR-FranciscaNeural",
            "ru" | "russian" | "ru-ru" => "ru-RU-SvetlanaNeural",
            _ => "en-US-JennyNeural",
        }
    }

    /// Get the current voice
    #[must_use]
    pub fn voice(&self) -> &str {
        &self.voice
    }

    /// Set a new voice
    pub fn set_voice(&mut self, voice: impl Into<String>) {
        self.voice = voice.into();
    }

    /// Synthesize text to audio (MP3 format)
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        if text.is_empty() {
            return Ok(Vec::new());
        }

        debug!(
            "Synthesizing: {} chars with voice {}",
            text.len(),
            self.voice
        );

        // Build SSML
        let ssml = format!(
            r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="en-US">
                <voice name="{}">{}</voice>
            </speak>"#,
            self.voice,
            Self::escape_xml(text)
        );

        // Build request
        let url = format!(
            "{}?trustedclienttoken={}",
            EDGE_TTS_ENDPOINT, TRUSTED_CLIENT_TOKEN
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/ssml+xml")
            .header(
                "X-Microsoft-OutputFormat",
                "audio-24khz-48kbitrate-mono-mp3",
            )
            .body(ssml)
            .send()
            .await
            .map_err(|e| Error::Tts(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Tts(format!("TTS API error: {}", response.status())));
        }

        let audio_bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Tts(format!("Failed to read response: {}", e)))?;

        debug!("Synthesized {} bytes of audio", audio_bytes.len());

        Ok(audio_bytes.to_vec())
    }

    /// Escape XML special characters
    fn escape_xml(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    /// List available Korean voices
    #[must_use]
    pub fn korean_voices() -> &'static [(&'static str, &'static str)] {
        &[
            ("ko-KR-SunHiNeural", "Korean Female (SunHi)"),
            ("ko-KR-InJoonNeural", "Korean Male (InJoon)"),
            ("ko-KR-YuJinNeural", "Korean Female (YuJin)"),
            ("ko-KR-HyunsuNeural", "Korean Male (Hyunsu)"),
        ]
    }

    /// List available English voices
    #[must_use]
    pub fn english_voices() -> &'static [(&'static str, &'static str)] {
        &[
            ("en-US-JennyNeural", "US English Female (Jenny)"),
            ("en-US-GuyNeural", "US English Male (Guy)"),
            ("en-US-AriaNeural", "US English Female (Aria)"),
            ("en-US-DavisNeural", "US English Male (Davis)"),
            ("en-GB-SoniaNeural", "British English Female (Sonia)"),
            ("en-GB-RyanNeural", "British English Male (Ryan)"),
        ]
    }

    /// List all supported languages
    #[must_use]
    pub fn supported_languages() -> &'static [(&'static str, &'static str)] {
        &[
            ("ko-KR", "Korean"),
            ("en-US", "English (US)"),
            ("en-GB", "English (UK)"),
            ("ja-JP", "Japanese"),
            ("zh-CN", "Chinese (Simplified)"),
            ("zh-TW", "Chinese (Traditional)"),
            ("es-ES", "Spanish"),
            ("fr-FR", "French"),
            ("de-DE", "German"),
            ("it-IT", "Italian"),
            ("pt-BR", "Portuguese (Brazil)"),
            ("ru-RU", "Russian"),
        ]
    }
}

#[async_trait::async_trait]
impl super::backend::TtsBackend for TextToSpeech {
    fn name(&self) -> &str {
        "edge-tts"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn list_voices(&self) -> crate::tts::error::Result<Vec<super::backend::VoiceInfo>> {
        let mut voices = Vec::new();
        for (id, name) in Self::korean_voices() {
            voices.push(super::backend::VoiceInfo {
                id: id.to_string(),
                name: name.to_string(),
                language: "ko".to_string(),
                gender: None,
                preview_url: None,
                labels: std::collections::HashMap::new(),
            });
        }
        for (id, name) in Self::english_voices() {
            voices.push(super::backend::VoiceInfo {
                id: id.to_string(),
                name: name.to_string(),
                language: "en".to_string(),
                gender: None,
                preview_url: None,
                labels: std::collections::HashMap::new(),
            });
        }
        Ok(voices)
    }

    async fn synthesize(
        &self,
        text: &str,
        voice: &str,
        _options: &super::backend::TtsOptions,
    ) -> crate::tts::error::Result<bytes::Bytes> {
        let tts = TextToSpeech::with_voice(voice);
        let audio = tts
            .synthesize(text)
            .await
            .map_err(|e| crate::tts::error::TtsError::EdgeError(e.to_string()))?;
        Ok(bytes::Bytes::from(audio))
    }

    async fn synthesize_stream(
        &self,
        text: &str,
        voice: &str,
        options: &super::backend::TtsOptions,
    ) -> crate::tts::error::Result<
        tokio::sync::mpsc::Receiver<crate::tts::error::Result<bytes::Bytes>>,
    > {
        let bytes =
            <Self as super::backend::TtsBackend>::synthesize(self, text, voice, options).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tx.send(Ok(bytes)).await.ok();
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_selection() {
        assert_eq!(
            TextToSpeech::get_voice_for_language("ko"),
            "ko-KR-SunHiNeural"
        );
        assert_eq!(
            TextToSpeech::get_voice_for_language("en"),
            "en-US-JennyNeural"
        );
        assert_eq!(
            TextToSpeech::get_voice_for_language("ja"),
            "ja-JP-NanamiNeural"
        );
        assert_eq!(
            TextToSpeech::get_voice_for_language("unknown"),
            "en-US-JennyNeural"
        );
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(
            TextToSpeech::escape_xml("Hello & World"),
            "Hello &amp; World"
        );
        assert_eq!(TextToSpeech::escape_xml("<script>"), "&lt;script&gt;");
    }

    #[test]
    fn test_korean_voices() {
        let voices = TextToSpeech::korean_voices();
        assert!(!voices.is_empty());
        assert!(voices.iter().any(|(v, _)| *v == "ko-KR-SunHiNeural"));
    }

    #[test]
    fn test_create_tts() {
        let tts = TextToSpeech::new("ko");
        assert!(tts.voice().contains("ko-KR"));

        let tts = TextToSpeech::with_voice("en-US-AriaNeural");
        assert_eq!(tts.voice(), "en-US-AriaNeural");
    }
}
