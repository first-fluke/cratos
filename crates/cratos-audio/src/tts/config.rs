use super::secret::SecretString as Secret;
use serde::Deserialize;
use std::env;

use super::error::Result;

#[derive(Debug, Deserialize, Clone)]
pub struct ElevenLabsConfig {
    pub model_id: String,
    pub default_voice: String,
    // api_key is handled separately via env/secret
    #[serde(skip)]
    pub api_key: Option<Secret>,
}

impl Default for ElevenLabsConfig {
    fn default() -> Self {
        Self {
            model_id: "eleven_multilingual_v2".to_string(),
            default_voice: "21m00Tcm4TlvDq8ikWAM".to_string(), // Rachel
            api_key: None,
        }
    }
}

impl ElevenLabsConfig {
    pub fn from_env() -> Result<Self> {
        let api_key = env::var("ELEVENLABS_API_KEY").ok().map(Secret::new);
        
        Ok(Self {
            model_id: env::var("ELEVENLABS_MODEL_ID").unwrap_or_else(|_| "eleven_multilingual_v2".to_string()),
            default_voice: env::var("ELEVENLABS_VOICE_ID").unwrap_or_else(|_| "21m00Tcm4TlvDq8ikWAM".to_string()),
            api_key,
        })
    }
}
