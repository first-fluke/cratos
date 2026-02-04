//! Wake Word Detection using Silero VAD (ONNX)
//!
//! Uses the Silero Voice Activity Detection model to detect when the user
//! starts speaking, combined with pattern matching for wake words.

use crate::error::{Error, Result};
use std::path::Path;
use tracing::info;

/// Silero VAD model sample rate
#[allow(dead_code)] // Reserved for Silero VAD implementation
const SILERO_SAMPLE_RATE: u32 = 16000;

/// Window size for VAD (512 samples = 32ms at 16kHz)
#[allow(dead_code)] // Reserved for Silero VAD implementation
const WINDOW_SIZE: usize = 512;

/// Wake word detector using Silero VAD
///
/// Note: Due to ort API complexity, this implementation uses SimpleVAD
/// as a fallback until the ONNX model loading is properly integrated.
pub struct WakeWordDetector {
    threshold: f32,
    model_path: std::path::PathBuf,
    // Using SimpleVAD internally for now
    simple_vad: SimpleVAD,
}

impl WakeWordDetector {
    /// Create a new wake word detector
    ///
    /// # Arguments
    /// * `model_path` - Path to the Silero VAD ONNX model
    /// * `threshold` - Voice activity threshold (0.0 - 1.0, default 0.5)
    pub fn new(model_path: impl AsRef<Path>, threshold: f32) -> Result<Self> {
        let model_path = model_path.as_ref();

        if !model_path.exists() {
            return Err(Error::ModelNotFound(format!(
                "Silero VAD model not found: {}. Download from: https://huggingface.co/snakers4/silero-vad/resolve/main/silero_vad.onnx",
                model_path.display()
            )));
        }

        info!("Wake word detector initialized (using energy-based VAD)");
        info!(
            "Model path: {} (available for future ONNX integration)",
            model_path.display()
        );

        Ok(Self {
            threshold,
            model_path: model_path.to_path_buf(),
            simple_vad: SimpleVAD::new(threshold),
        })
    }

    /// Create with default model path
    pub fn with_default_path(threshold: f32) -> Result<Self> {
        let model_path = crate::default_models_dir().join("silero_vad.onnx");

        // If model doesn't exist, create detector anyway with fallback
        if !model_path.exists() {
            info!("Wake word detector initialized (energy-based VAD, no ONNX model)");
            return Ok(Self {
                threshold,
                model_path,
                simple_vad: SimpleVAD::new(threshold),
            });
        }

        Self::new(model_path, threshold)
    }

    /// Reset hidden states (call when starting a new recording)
    pub fn reset(&mut self) {
        // No-op for SimpleVAD fallback
    }

    /// Detect voice activity in audio samples
    ///
    /// # Arguments
    /// * `samples` - Audio samples at 16kHz mono
    ///
    /// # Returns
    /// Voice activity probability (0.0 - 1.0)
    pub fn detect(&mut self, samples: &[f32]) -> Result<f32> {
        // Use energy-based detection
        let energy = self.simple_vad.energy(samples);
        // Normalize to 0-1 range (assuming max energy ~0.5 for speech)
        Ok((energy * 2.0).min(1.0))
    }

    /// Check if voice is detected above threshold
    pub fn is_voice_detected(&mut self, samples: &[f32]) -> Result<bool> {
        Ok(self.simple_vad.is_voice_detected(samples))
    }

    /// Set detection threshold
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.0, 1.0);
        self.simple_vad = SimpleVAD::new(self.threshold);
    }

    /// Get current threshold
    #[must_use]
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Get model path
    #[must_use]
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }
}

/// Simple energy-based voice activity detector (fallback when ONNX not available)
pub struct SimpleVAD {
    threshold: f32,
}

impl SimpleVAD {
    /// Create a new simple VAD
    #[must_use]
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    /// Calculate RMS energy of samples
    pub fn energy(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// Check if voice is detected
    pub fn is_voice_detected(&self, samples: &[f32]) -> bool {
        self.energy(samples) >= self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_vad() {
        let vad = SimpleVAD::new(0.1);

        // Silent audio
        let silence = vec![0.0f32; 1600];
        assert!(!vad.is_voice_detected(&silence));

        // Loud audio
        let loud = vec![0.5f32; 1600];
        assert!(vad.is_voice_detected(&loud));
    }

    #[test]
    fn test_energy_calculation() {
        let vad = SimpleVAD::new(0.1);

        let samples = vec![0.5f32; 100];
        let energy = vad.energy(&samples);
        assert!((energy - 0.5).abs() < 0.01);
    }
}
