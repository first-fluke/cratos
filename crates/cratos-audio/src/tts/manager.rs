use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

use super::{
    backend::{TtsBackend, TtsOptions, VoiceInfo},
    error::{Result, TtsError},
};

#[derive(Clone)]
pub struct TtsManager {
    backends: HashMap<String, Arc<dyn TtsBackend>>,
    default_backend: String,
    fallback_enabled: bool,
    #[allow(dead_code)]
    voice_cache: Arc<RwLock<HashMap<String, Vec<VoiceInfo>>>>,
}

impl Default for TtsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsManager {
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            default_backend: String::new(),
            fallback_enabled: true,
            voice_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add_backend<B: TtsBackend + 'static>(mut self, backend: B) -> Self {
        let name = backend.name().to_string();
        if self.default_backend.is_empty() {
            self.default_backend = name.clone();
        }
        self.backends.insert(name, Arc::new(backend));
        self
    }

    pub fn set_default_backend(mut self, name: &str) -> Self {
        if self.backends.contains_key(name) {
            self.default_backend = name.to_string();
        }
        self
    }

    pub fn get_backend(&self, name: &str) -> Option<Arc<dyn TtsBackend>> {
        self.backends.get(name).cloned()
    }

    pub async fn synthesize(
        &self,
        text: &str,
        voice: &str,
        options: Option<TtsOptions>,
    ) -> Result<Bytes> {
        let options = options.unwrap_or_default();

        // Try default backend
        if let Some(backend) = self.get_backend(&self.default_backend) {
            if backend.is_available() {
                match backend.synthesize(text, voice, &options).await {
                    Ok(bytes) => return Ok(bytes),
                    Err(e) => {
                        if !self.fallback_enabled {
                            return Err(e);
                        }
                        warn!(
                            "Default backend {} failed: {}, trying fallback",
                            self.default_backend, e
                        );
                    }
                }
            }
        }

        // Fallback to any available backend
        for (name, backend) in &self.backends {
            if name == &self.default_backend || !backend.is_available() {
                continue;
            }
            // Map voice ID for fallback backend
            let fallback_voice = self.map_voice_for_backend(name, voice);

            if let Ok(bytes) = backend.synthesize(text, &fallback_voice, &options).await {
                return Ok(bytes);
            }
        }

        Err(TtsError::AllBackendsFailed)
    }

    fn map_voice_for_backend(&self, target_backend: &str, voice: &str) -> String {
        // Simple static mapping for MVP fallback
        match (target_backend, voice) {
            // ElevenLabs -> Edge (Rachel -> Aria, Antoni -> Guy)
            ("edge-tts", "21m00Tcm4TlvDq8ikWAM") => "en-US-AriaNeural".to_string(),
            ("edge-tts", "ErXwobaYiN019PkySvjV") => "en-US-GuyNeural".to_string(),

            // Edge -> ElevenLabs (Aria -> Rachel, Guy -> Antoni)
            ("elevenlabs", "en-US-AriaNeural") => "21m00Tcm4TlvDq8ikWAM".to_string(),
            ("elevenlabs", "en-US-GuyNeural") => "ErXwobaYiN019PkySvjV".to_string(),

            // Default to same ID if not mapped
            _ => voice.to_string(),
        }
    }
}
