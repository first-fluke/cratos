//! Local Whisper STT using candle (pure Rust, no C++ dependencies)
//!
//! Downloads and runs OpenAI Whisper models locally via HuggingFace Hub.
//! Supports tiny, base, and small model sizes.
//!
//! # Feature
//!
//! Requires the `local-stt` feature flag.

use crate::error::{Error, Result};
use candle_core::{Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, audio, Config};
use hf_hub::api::sync::Api;
use std::path::PathBuf;
use tokenizers::Tokenizer;
use tracing::{debug, info};

/// Whisper model size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhisperModel {
    /// ~75MB, fastest, lower accuracy
    Tiny,
    /// ~142MB, balanced speed/accuracy
    Base,
    /// ~466MB, best accuracy for Korean
    Small,
}

impl WhisperModel {
    /// HuggingFace model ID
    fn repo_id(self) -> &'static str {
        match self {
            Self::Tiny => "openai/whisper-tiny",
            Self::Base => "openai/whisper-base",
            Self::Small => "openai/whisper-small",
        }
    }

    /// Parse from string
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "tiny" => Self::Tiny,
            "base" => Self::Base,
            _ => Self::Small,
        }
    }

    /// Display name
    pub fn name(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Base => "base",
            Self::Small => "small",
        }
    }
}

/// Loaded whisper model internals
struct WhisperInner {
    model: m::model::Whisper,
    tokenizer: Tokenizer,
    config: Config,
    device: Device,
    mel_filters: Vec<f64>,
}

/// Local Whisper STT engine
pub struct LocalWhisper {
    model_size: WhisperModel,
    language: String,
    inner: Option<WhisperInner>,
}

impl LocalWhisper {
    /// Create a new local whisper engine (lazy - does not load model yet)
    #[must_use]
    pub fn new(model_size: WhisperModel, language: &str) -> Self {
        info!(
            "LocalWhisper created (model: {}, language: {}) - not loaded yet",
            model_size.name(),
            language
        );
        Self {
            model_size,
            language: language.to_string(),
            inner: None,
        }
    }

    /// Check if model is loaded
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.inner.is_some()
    }

    /// Load the model (downloads from HuggingFace on first run)
    pub fn load(&mut self) -> Result<()> {
        if self.inner.is_some() {
            return Ok(());
        }

        info!(
            "Loading Whisper {} model from HuggingFace...",
            self.model_size.name()
        );

        let device = Device::Cpu;
        let repo_id = self.model_size.repo_id();

        let api = Api::new()
            .map_err(|e| Error::ModelDownload(format!("Failed to create HF API: {e}")))?;
        let repo = api.model(repo_id.to_string());

        // Download model files
        info!("Downloading model weights...");
        let model_path = repo
            .get("model.safetensors")
            .map_err(|e| Error::ModelDownload(format!("Failed to download model: {e}")))?;

        info!("Downloading tokenizer...");
        let tokenizer_path = repo
            .get("tokenizer.json")
            .map_err(|e| Error::ModelDownload(format!("Failed to download tokenizer: {e}")))?;

        info!("Downloading config...");
        let config_path = repo
            .get("config.json")
            .map_err(|e| Error::ModelDownload(format!("Failed to download config: {e}")))?;

        // Load config
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| Error::ModelDownload(format!("Failed to read config: {e}")))?;
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| Error::ModelDownload(format!("Failed to parse config: {e}")))?;

        // Compute mel filterbank (standard Whisper uses 80 bins, large-v3 uses 128)
        let mel_filters = compute_mel_filters(config.num_mel_bins);

        // Load model
        info!("Loading model into memory...");
        let model_bytes = std::fs::read(&model_path)
            .map_err(|e| Error::Stt(format!("Failed to read model file: {e}")))?;
        let vb =
            VarBuilder::from_buffered_safetensors(model_bytes, candle_core::DType::F32, &device)
                .map_err(|e| Error::Stt(format!("Failed to load safetensors: {e}")))?;
        let model = m::model::Whisper::load(&vb, config.clone())
            .map_err(|e| Error::Stt(format!("Failed to load Whisper model: {e}")))?;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| Error::Stt(format!("Failed to load tokenizer: {e}")))?;

        info!(
            "Whisper {} loaded successfully ({} mel bins)",
            self.model_size.name(),
            config.num_mel_bins
        );

        self.inner = Some(WhisperInner {
            model,
            tokenizer,
            config,
            device,
            mel_filters,
        });

        Ok(())
    }

    /// Transcribe WAV audio bytes to text (synchronous, CPU-bound)
    pub fn transcribe_sync(&mut self, audio_bytes: &[u8]) -> Result<String> {
        self.load()?;

        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| Error::Stt("Model not loaded".to_string()))?;

        // Decode WAV to PCM f64 samples at 16kHz
        let pcm = wav_to_pcm(audio_bytes)?;
        debug!(
            "PCM samples: {}, duration: {:.1}s",
            pcm.len(),
            pcm.len() as f64 / 16000.0
        );

        // Convert PCM to mel spectrogram
        let mel = audio::pcm_to_mel(&inner.config, &pcm, &inner.mel_filters);
        let mel_len = mel.len() / inner.config.num_mel_bins;

        // Convert f64 mel to f32 for tensor
        let mel_f32: Vec<f32> = mel.iter().map(|&v| v as f32).collect();

        let mel_tensor = Tensor::from_vec(
            mel_f32,
            (1, inner.config.num_mel_bins, mel_len),
            &inner.device,
        )
        .map_err(|e| Error::Stt(format!("Failed to create mel tensor: {e}")))?;

        // Encode audio
        let encoder_output = inner
            .model
            .encoder
            .forward(&mel_tensor, true)
            .map_err(|e| Error::Stt(format!("Encoder forward failed: {e}")))?;

        // Prepare decoder tokens
        let sot_token = token_id(&inner.tokenizer, "<|startoftranscript|>")?;
        let transcribe_token = token_id(&inner.tokenizer, "<|transcribe|>")?;
        let no_timestamps_token = token_id(&inner.tokenizer, "<|notimestamps|>")?;
        let eot_token = token_id(&inner.tokenizer, "<|endoftext|>")?;

        // Language token
        let lang_token =
            token_id(&inner.tokenizer, &format!("<|{}|>", &self.language)).unwrap_or(sot_token);

        let mut tokens = vec![sot_token, lang_token, transcribe_token, no_timestamps_token];
        let mut result_tokens: Vec<u32> = Vec::new();

        // Greedy decoding loop
        let max_tokens = 224; // Whisper maximum text tokens
        inner.model.reset_kv_cache();

        for _i in 0..max_tokens {
            let token_tensor = Tensor::new(tokens.as_slice(), &inner.device)
                .map_err(|e| Error::Stt(format!("Failed to create token tensor: {e}")))?
                .unsqueeze(0)
                .map_err(|e| Error::Stt(format!("Failed to unsqueeze: {e}")))?;

            let logits = inner
                .model
                .decoder
                .forward(&token_tensor, &encoder_output, true)
                .map_err(|e| Error::Stt(format!("Decoder forward failed: {e}")))?;

            let logits = inner
                .model
                .decoder
                .final_linear(&logits)
                .map_err(|e| Error::Stt(format!("Final linear failed: {e}")))?;

            // Get last token logits
            let seq_len = logits
                .dim(1)
                .map_err(|e| Error::Stt(format!("Dim error: {e}")))?;
            let last_logits = logits
                .i((0, seq_len - 1, ..))
                .map_err(|e| Error::Stt(format!("Index error: {e}")))?;

            // Argmax (greedy decoding)
            let next_token = last_logits
                .argmax(0)
                .map_err(|e| Error::Stt(format!("Argmax error: {e}")))?
                .to_scalar::<u32>()
                .map_err(|e| Error::Stt(format!("Scalar error: {e}")))?;

            if next_token == eot_token {
                break;
            }

            result_tokens.push(next_token);
            tokens = vec![next_token];
        }

        // Decode tokens to text
        let text = inner
            .tokenizer
            .decode(&result_tokens, true)
            .map_err(|e| Error::Stt(format!("Token decoding failed: {e}")))?;

        let text = text.trim().to_string();
        debug!("Transcription: {}", text);

        Ok(text)
    }

    /// Get model size
    #[must_use]
    pub fn model_size(&self) -> WhisperModel {
        self.model_size
    }

    /// Get the model cache directory
    #[must_use]
    pub fn cache_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cache")
            .join("huggingface")
    }
}

/// Look up a token ID from the tokenizer
fn token_id(tokenizer: &Tokenizer, token: &str) -> Result<u32> {
    tokenizer
        .token_to_id(token)
        .ok_or_else(|| Error::Stt(format!("Token not found: {token}")))
}

/// Compute mel filterbank coefficients
///
/// Implements the standard mel filterbank used by OpenAI Whisper.
/// This matches the `librosa.filters.mel` function with:
/// - sample_rate = 16000
/// - n_fft = 400
/// - n_mels = num_mel_bins (80 or 128)
/// - fmin = 0
/// - fmax = 8000
fn compute_mel_filters(num_mel_bins: usize) -> Vec<f64> {
    let sample_rate = 16000.0_f64;
    let n_fft = 400;
    let fmin = 0.0_f64;
    let fmax = sample_rate / 2.0; // 8000 Hz
    let n_freqs = n_fft / 2 + 1; // 201

    // Hz to mel conversion (HTK formula)
    let hz_to_mel = |hz: f64| -> f64 { 2595.0 * (1.0 + hz / 700.0).log10() };
    let mel_to_hz = |mel: f64| -> f64 { 700.0 * (10.0_f64.powf(mel / 2595.0) - 1.0) };

    let mel_min = hz_to_mel(fmin);
    let mel_max = hz_to_mel(fmax);

    // Create num_mel_bins + 2 equally spaced points in mel space
    let n_points = num_mel_bins + 2;
    let mel_points: Vec<f64> = (0..n_points)
        .map(|i| mel_min + (mel_max - mel_min) * i as f64 / (n_points - 1) as f64)
        .collect();

    // Convert back to Hz
    let hz_points: Vec<f64> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();

    // Convert to FFT bin indices
    let fft_freqs: Vec<f64> = (0..n_freqs)
        .map(|i| sample_rate * i as f64 / n_fft as f64)
        .collect();

    // Build filterbank: shape [num_mel_bins, n_freqs] stored row-major
    let mut filters = vec![0.0f64; num_mel_bins * n_freqs];

    for mel_idx in 0..num_mel_bins {
        let left = hz_points[mel_idx];
        let center = hz_points[mel_idx + 1];
        let right = hz_points[mel_idx + 2];

        for freq_idx in 0..n_freqs {
            let freq = fft_freqs[freq_idx];

            if freq >= left && freq <= center && center > left {
                filters[mel_idx * n_freqs + freq_idx] = (freq - left) / (center - left);
            } else if freq > center && freq <= right && right > center {
                filters[mel_idx * n_freqs + freq_idx] = (right - freq) / (right - center);
            }
        }

        // Slaney normalization: 2.0 / (hz_points[mel_idx+2] - hz_points[mel_idx])
        let enorm = 2.0 / (hz_points[mel_idx + 2] - hz_points[mel_idx]);
        for freq_idx in 0..n_freqs {
            filters[mel_idx * n_freqs + freq_idx] *= enorm;
        }
    }

    filters
}

/// Decode WAV bytes to f64 PCM samples at 16kHz
fn wav_to_pcm(wav_bytes: &[u8]) -> Result<Vec<f64>> {
    let cursor = std::io::Cursor::new(wav_bytes);
    let reader = hound::WavReader::new(cursor)
        .map_err(|e| Error::Stt(format!("Failed to read WAV: {e}")))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    // Read samples as f64
    let samples: Vec<f64> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .map(f64::from)
            .collect(),
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_val = (1u64 << (bits - 1)) as f64;
            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| f64::from(s) / max_val)
                .collect()
        }
    };

    // Handle stereo -> mono
    let samples = if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f64>() / chunk.len() as f64)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if needed
    if sample_rate != 16000 {
        debug!("Resampling from {}Hz to 16000Hz", sample_rate);
        Ok(resample(&samples, sample_rate, 16000))
    } else {
        Ok(samples)
    }
}

/// Simple linear resampling
fn resample(samples: &[f64], from_rate: u32, to_rate: u32) -> Vec<f64> {
    let ratio = f64::from(to_rate) / f64::from(from_rate);
    let new_len = (samples.len() as f64 * ratio) as usize;
    let mut resampled = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 / ratio;
        let idx = src_idx as usize;
        let frac = src_idx - idx as f64;

        if idx + 1 < samples.len() {
            resampled.push(samples[idx] * (1.0 - frac) + samples[idx + 1] * frac);
        } else if idx < samples.len() {
            resampled.push(samples[idx]);
        }
    }

    resampled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_model_from_str() {
        assert_eq!(WhisperModel::parse("tiny"), WhisperModel::Tiny);
        assert_eq!(WhisperModel::parse("base"), WhisperModel::Base);
        assert_eq!(WhisperModel::parse("small"), WhisperModel::Small);
        assert_eq!(WhisperModel::parse("unknown"), WhisperModel::Small);
    }

    #[test]
    fn test_whisper_model_repo_id() {
        assert_eq!(WhisperModel::Tiny.repo_id(), "openai/whisper-tiny");
        assert_eq!(WhisperModel::Base.repo_id(), "openai/whisper-base");
        assert_eq!(WhisperModel::Small.repo_id(), "openai/whisper-small");
    }

    #[test]
    fn test_local_whisper_creation() {
        let whisper = LocalWhisper::new(WhisperModel::Tiny, "ko");
        assert!(!whisper.is_loaded());
        assert_eq!(whisper.model_size(), WhisperModel::Tiny);
    }

    #[test]
    fn test_resample() {
        // Simple test: resample 4 samples from 8kHz to 16kHz
        let input = vec![0.0, 1.0, 0.0, -1.0];
        let output = resample(&input, 8000, 16000);
        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_compute_mel_filters_80() {
        let filters = compute_mel_filters(80);
        // 80 mel bins * 201 frequency bins
        assert_eq!(filters.len(), 80 * 201);
        // All values should be non-negative
        assert!(filters.iter().all(|&v| v >= 0.0));
        // At least some values should be positive
        assert!(filters.iter().any(|&v| v > 0.0));
    }

    #[test]
    fn test_compute_mel_filters_128() {
        let filters = compute_mel_filters(128);
        assert_eq!(filters.len(), 128 * 201);
        assert!(filters.iter().all(|&v| v >= 0.0));
        assert!(filters.iter().any(|&v| v > 0.0));
    }
}
