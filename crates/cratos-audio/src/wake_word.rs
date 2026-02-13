//! Wake Word Detection using Silero VAD (ONNX)
//!
//! Uses the Silero Voice Activity Detection model to detect when the user
//! starts speaking, combined with pattern matching for wake words.
//!
//! With `silero-vad` feature: uses tract-onnx for real ONNX inference.
//! Without: uses energy-based SimpleVAD as fallback.

use crate::error::Result;
use std::path::Path;

/// Window size for VAD (512 samples = 32ms at 16kHz)
#[cfg(any(feature = "silero-vad", test))]
const WINDOW_SIZE: usize = 512;

#[cfg(feature = "silero-vad")]
mod vad_constants {
    /// Silero VAD model sample rate
    pub const SILERO_SAMPLE_RATE: u32 = 16000;
    /// Context size prepended to each chunk (64 samples at 16kHz)
    pub const CONTEXT_SIZE: usize = 64;
    /// Hidden state dimension for Silero VAD LSTM
    pub const STATE_DIM: usize = 128;
}

// ─── Silero VAD ONNX backend ──────────────────────────────────

#[cfg(feature = "silero-vad")]
mod silero {
    use super::vad_constants::*;
    use super::*;
    use crate::error::Error;
    use tracing::info;
    use tract_onnx::prelude::*;

    type SileroModel = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

    /// Silero VAD model loaded via tract-onnx.
    pub struct SileroVAD {
        model: SileroModel,
        state: tract_ndarray::Array3<f32>,
        context: Vec<f32>,
    }

    impl SileroVAD {
        /// Load the Silero VAD ONNX model from disk.
        pub fn load(model_path: &Path) -> Result<Self> {
            let model = tract_onnx::onnx()
                .model_for_path(model_path)
                .map_err(|e| Error::Config(format!("Failed to load Silero VAD model: {}", e)))?
                .with_input_fact(
                    0,
                    InferenceFact::dt_shape(
                        f32::datum_type(),
                        tvec![1, (WINDOW_SIZE + CONTEXT_SIZE) as i64],
                    ),
                )
                .map_err(|e| Error::Config(format!("Input fact error: {}", e)))?
                .with_input_fact(
                    1,
                    InferenceFact::dt_shape(f32::datum_type(), tvec![2, 1, STATE_DIM as i64]),
                )
                .map_err(|e| Error::Config(format!("State fact error: {}", e)))?
                .with_input_fact(2, InferenceFact::dt_shape(i64::datum_type(), tvec![1]))
                .map_err(|e| Error::Config(format!("SR fact error: {}", e)))?
                .into_optimized()
                .map_err(|e| Error::Config(format!("Model optimize error: {}", e)))?
                .into_runnable()
                .map_err(|e| Error::Config(format!("Model runnable error: {}", e)))?;

            info!("Silero VAD ONNX model loaded from {}", model_path.display());

            Ok(Self {
                model,
                state: tract_ndarray::Array3::<f32>::zeros((2, 1, STATE_DIM)),
                context: vec![0.0f32; CONTEXT_SIZE],
            })
        }

        /// Download the model from HuggingFace if not present.
        pub fn download_model() -> Result<std::path::PathBuf> {
            use hf_hub::api::sync::Api;

            let api =
                Api::new().map_err(|e| Error::Config(format!("HuggingFace API error: {}", e)))?;
            let repo = api.model("snakers4/silero-vad".to_string());
            let path = repo
                .get("silero_vad.onnx")
                .map_err(|e| Error::Config(format!("Model download error: {}", e)))?;
            info!("Silero VAD model available at {}", path.display());
            Ok(path)
        }

        /// Reset hidden states (call when starting a new recording).
        pub fn reset(&mut self) {
            self.state = tract_ndarray::Array3::<f32>::zeros((2, 1, STATE_DIM));
            self.context = vec![0.0f32; CONTEXT_SIZE];
        }

        /// Run inference on a chunk of audio samples.
        ///
        /// Returns speech probability (0.0 - 1.0).
        pub fn infer(&mut self, samples: &[f32]) -> Result<f32> {
            // Build input: context + samples
            let mut input_data = Vec::with_capacity(CONTEXT_SIZE + samples.len());
            input_data.extend_from_slice(&self.context);
            input_data.extend_from_slice(samples);

            let input_len = input_data.len();
            let input_tensor: Tensor =
                tract_ndarray::Array2::from_shape_vec((1, input_len), input_data)
                    .map_err(|e| Error::Config(format!("Input tensor error: {}", e)))?
                    .into();

            let state_tensor: Tensor = self.state.clone().into();

            let sr_tensor: Tensor = tract_ndarray::arr1(&[SILERO_SAMPLE_RATE as i64]).into();

            let outputs = self
                .model
                .run(tvec![
                    input_tensor.into(),
                    state_tensor.into(),
                    sr_tensor.into(),
                ])
                .map_err(|e| Error::Config(format!("Silero inference error: {}", e)))?;

            // Output[0]: speech probability
            let prob = outputs[0]
                .to_array_view::<f32>()
                .map_err(|e| Error::Config(format!("Output parse error: {}", e)))?;
            let speech_prob = prob.iter().next().copied().unwrap_or(0.0);

            // Output[1]: updated state
            if let Ok(new_state) = outputs[1].to_array_view::<f32>() {
                if let Ok(s) = new_state.to_shape((2, 1, STATE_DIM)) {
                    self.state = s.to_owned();
                }
            }

            // Update context from the end of the current samples
            if samples.len() >= CONTEXT_SIZE {
                self.context
                    .copy_from_slice(&samples[samples.len() - CONTEXT_SIZE..]);
            }

            Ok(speech_prob)
        }
    }
}

// ─── WakeWordDetector (public API) ────────────────────────────

/// Wake word detector using Silero VAD (ONNX) or energy-based fallback.
pub struct WakeWordDetector {
    threshold: f32,
    model_path: std::path::PathBuf,
    #[cfg(feature = "silero-vad")]
    silero: Option<silero::SileroVAD>,
    simple_vad: SimpleVAD,
}

impl WakeWordDetector {
    /// Create a new wake word detector.
    ///
    /// With `silero-vad` feature, loads the ONNX model from `model_path`.
    /// Falls back to energy-based VAD if loading fails.
    pub fn new(model_path: impl AsRef<Path>, threshold: f32) -> Result<Self> {
        let model_path = model_path.as_ref();

        #[cfg(feature = "silero-vad")]
        {
            if model_path.exists() {
                match silero::SileroVAD::load(model_path) {
                    Ok(vad) => {
                        tracing::info!("Wake word detector initialized (Silero VAD ONNX)");
                        return Ok(Self {
                            threshold,
                            model_path: model_path.to_path_buf(),
                            silero: Some(vad),
                            simple_vad: SimpleVAD::new(threshold),
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Silero VAD load failed, falling back to energy-based VAD"
                        );
                    }
                }
            }
        }

        if !model_path.exists() {
            tracing::info!("Wake word detector initialized (energy-based VAD, no ONNX model)");
        } else {
            tracing::info!("Wake word detector initialized (energy-based VAD)");
        }

        Ok(Self {
            threshold,
            model_path: model_path.to_path_buf(),
            #[cfg(feature = "silero-vad")]
            silero: None,
            simple_vad: SimpleVAD::new(threshold),
        })
    }

    /// Create with default model path, downloading if needed.
    pub fn with_default_path(threshold: f32) -> Result<Self> {
        let model_path = crate::default_models_dir().join("silero_vad.onnx");

        #[cfg(feature = "silero-vad")]
        if !model_path.exists() {
            match silero::SileroVAD::download_model() {
                Ok(downloaded) => return Self::new(downloaded, threshold),
                Err(e) => {
                    tracing::warn!(error = %e, "Silero VAD download failed, using fallback");
                }
            }
        }

        if model_path.exists() {
            Self::new(&model_path, threshold)
        } else {
            Ok(Self {
                threshold,
                model_path,
                #[cfg(feature = "silero-vad")]
                silero: None,
                simple_vad: SimpleVAD::new(threshold),
            })
        }
    }

    /// Reset hidden states (call when starting a new recording).
    pub fn reset(&mut self) {
        #[cfg(feature = "silero-vad")]
        if let Some(ref mut s) = self.silero {
            s.reset();
        }
    }

    /// Detect voice activity in audio samples.
    ///
    /// Returns voice activity probability (0.0 - 1.0).
    pub fn detect(&mut self, samples: &[f32]) -> Result<f32> {
        #[cfg(feature = "silero-vad")]
        if let Some(ref mut s) = self.silero {
            return s.infer(samples);
        }

        // Fallback: energy-based detection
        let energy = self.simple_vad.energy(samples);
        Ok((energy * 2.0).min(1.0))
    }

    /// Check if voice is detected above threshold.
    pub fn is_voice_detected(&mut self, samples: &[f32]) -> Result<bool> {
        #[cfg(feature = "silero-vad")]
        if let Some(ref mut s) = self.silero {
            let prob = s.infer(samples)?;
            return Ok(prob >= self.threshold);
        }

        Ok(self.simple_vad.is_voice_detected(samples))
    }

    /// Set detection threshold.
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.0, 1.0);
        self.simple_vad = SimpleVAD::new(self.threshold);
    }

    /// Get current threshold.
    #[must_use]
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Get model path.
    #[must_use]
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Whether the Silero ONNX backend is active.
    #[must_use]
    pub fn is_silero_active(&self) -> bool {
        #[cfg(feature = "silero-vad")]
        {
            self.silero.is_some()
        }
        #[cfg(not(feature = "silero-vad"))]
        {
            false
        }
    }
}

/// Simple energy-based voice activity detector (fallback).
pub struct SimpleVAD {
    threshold: f32,
}

impl SimpleVAD {
    /// Create a new simple VAD.
    #[must_use]
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    /// Calculate RMS energy of samples.
    pub fn energy(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// Check if voice is detected.
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

    #[test]
    fn test_wake_word_detector_fallback() {
        let mut detector = WakeWordDetector::with_default_path(0.5).unwrap();

        // Without the model, should use SimpleVAD fallback
        let silence = vec![0.0f32; WINDOW_SIZE];
        let prob = detector.detect(&silence).unwrap();
        assert!(prob < 0.1);

        let loud = vec![0.5f32; WINDOW_SIZE];
        let detected = detector.is_voice_detected(&loud).unwrap();
        assert!(detected);
    }

    #[test]
    fn test_threshold_clamping() {
        let mut detector = WakeWordDetector::with_default_path(0.5).unwrap();
        detector.set_threshold(1.5);
        assert!((detector.threshold() - 1.0).abs() < f32::EPSILON);

        detector.set_threshold(-0.5);
        assert!(detector.threshold().abs() < f32::EPSILON);
    }

    #[test]
    fn test_is_silero_active() {
        let detector = WakeWordDetector::with_default_path(0.5).unwrap();
        // Without the silero-vad feature or model, should be false
        #[cfg(not(feature = "silero-vad"))]
        assert!(!detector.is_silero_active());
    }
}
