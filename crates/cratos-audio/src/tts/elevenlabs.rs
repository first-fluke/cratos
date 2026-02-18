use async_trait::async_trait;
use bytes::Bytes;
use reqwest::Client;
use super::secret::SecretString as Secret;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::warn;

use super::{
    backend::{TtsBackend, TtsOptions, VoiceInfo},
    config::ElevenLabsConfig,
    error::{Result, TtsError},
    rate_limit::{ElevenLabsRateLimiter, ElevenLabsTier},
};

const API_BASE: &str = "https://api.elevenlabs.io/v1";

#[derive(Debug, Serialize)]
struct ElevenLabsTtsRequest {
    text: String,
    model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice_settings: Option<ElevenLabsVoiceSettings>,
}

#[derive(Debug, Serialize)]
struct ElevenLabsVoiceSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    stability: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    similarity_boost: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    use_speaker_boost: Option<bool>,
}

#[derive(Clone)]
pub struct ElevenLabsBackend {
    client: Client,
    config: ElevenLabsConfig,
    rate_limiter: Arc<ElevenLabsRateLimiter>,
}

impl ElevenLabsBackend {
    pub fn new(config: ElevenLabsConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            // Assume Free tier by default, or configure via env if needed
            rate_limiter: Arc::new(ElevenLabsRateLimiter::new(ElevenLabsTier::Free)), 
        }
    }

    fn get_api_key(&self) -> Result<&Secret> {
        self.config.api_key.as_ref().ok_or(TtsError::ApiKeyNotFound)
    }
}

#[async_trait]
impl TtsBackend for ElevenLabsBackend {
    fn name(&self) -> &str {
        "elevenlabs"
    }

    fn is_available(&self) -> bool {
        self.config.api_key.is_some()
    }

    async fn list_voices(&self) -> Result<Vec<VoiceInfo>> {
        let url = format!("{}/voices", API_BASE);
        let api_key = self.get_api_key()?;
        
        // This is a simplified response struct for listing voices
        #[derive(Deserialize)]
        struct VoiceListResponse {
            voices: Vec<VoiceDetails>,
        }
        
        #[derive(Deserialize)]
        struct VoiceDetails {
            voice_id: String,
            name: String,
            // Add other fields as needed
        }

        let resp = self.client.get(&url)
            .header("xi-api-key", api_key.expose_secret())
            .send().await?
            .error_for_status()?;
            
        let data: VoiceListResponse = resp.json().await?;
        
        let voices = data.voices.into_iter().map(|v| VoiceInfo {
            id: v.voice_id,
            name: v.name,
            language: "en".to_string(), // Default assumption, actual API returns labels
            gender: None, // Need to parse labels for gender
            preview_url: None,
            labels: std::collections::HashMap::new(),
        }).collect();
        
        Ok(voices)
    }

    async fn synthesize(&self, text: &str, voice: &str, options: &TtsOptions) -> Result<Bytes> {
        let api_key = self.get_api_key()?;
        
        // Check rate limit
        self.rate_limiter.check(text.len()).await?;
        
        let url = format!("{}/text-to-speech/{}", API_BASE, voice);
        
        let body = ElevenLabsTtsRequest {
            text: text.to_string(),
            model_id: self.config.model_id.clone(),
            voice_settings: Some(ElevenLabsVoiceSettings {
                stability: options.stability,
                similarity_boost: options.similarity_boost,
                style: options.style,
                use_speaker_boost: Some(true),
            }),
        };
        
        let resp = self.client.post(&url)
            .header("xi-api-key", api_key.expose_secret())
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await?;
            
        // Check rate limit headers for logging
        if let Some(remaining) = resp.headers().get("x-ratelimit-remaining") {
            if let Ok(val) = remaining.to_str() {
                if let Ok(rem) = val.parse::<i32>() {
                    if rem < 10 {
                        warn!("ElevenLabs rate limit low: {}", rem);
                    }
                }
            }
        }
        
        let bytes = resp.error_for_status()?.bytes().await?;
        Ok(bytes)
    }
    
    async fn synthesize_stream(&self, text: &str, voice: &str, options: &TtsOptions) 
        -> Result<mpsc::Receiver<Result<Bytes>>> 
    {
        let api_key = self.get_api_key()?;
        
        // Check rate limit
        self.rate_limiter.check(text.len()).await?;

        let url = format!("{}/text-to-speech/{}/stream", API_BASE, voice);
        
        let body = ElevenLabsTtsRequest {
            text: text.to_string(),
            model_id: self.config.model_id.clone(),
            voice_settings: Some(ElevenLabsVoiceSettings {
                stability: options.stability,
                similarity_boost: options.similarity_boost,
                style: options.style,
                use_speaker_boost: Some(true),
            }),
        };

        let resp = self.client.post(&url)
            .header("xi-api-key", api_key.expose_secret())
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await?
            .error_for_status()?;
            
        let (tx, rx) = mpsc::channel(32);
        
        tokio::spawn(async move {
            let mut stream = resp.bytes_stream();
            while let Some(item) = stream.next().await {
                // Map reqwest error to TtsError
                let chunk = item.map_err(TtsError::from);
                if tx.send(chunk).await.is_err() {
                    break;
                }
            }
        });
        
        Ok(rx)
    }
}
