use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

use super::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VoiceGender {
    Male,
    Female,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub language: String,
    pub gender: Option<VoiceGender>,
    pub preview_url: Option<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct TtsOptions {
    pub stability: Option<f32>,        // 0.0-1.0
    pub similarity_boost: Option<f32>, // 0.0-1.0
    pub style: Option<f32>,            // 0.0-1.0
    pub speed: Option<f32>,            // 0.5-2.0
    pub pitch: Option<i8>,             // -20 to 20 semitones
    pub output_format: OutputFormat,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Mp3,
    Wav,
    Ogg,
    Pcm,
}

#[async_trait]
pub trait TtsBackend: Send + Sync {
    fn name(&self) -> &str;

    fn is_available(&self) -> bool {
        true
    }

    async fn list_voices(&self) -> Result<Vec<VoiceInfo>>;

    async fn synthesize(&self, text: &str, voice: &str, options: &TtsOptions) -> Result<Bytes>;

    async fn synthesize_stream(
        &self,
        text: &str,
        voice: &str,
        options: &TtsOptions,
    ) -> Result<mpsc::Receiver<Result<Bytes>>>;
}
