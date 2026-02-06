//! Voice Controller - Unified voice interface
//!
//! Integrates wake word detection, STT, and TTS into a unified controller.

use crate::config::VoiceConfig;
use crate::error::{Error, Result};
use crate::input::{samples_to_wav, AudioInput};
use crate::output::AudioOutput;
use crate::stt::SpeechToText;
use crate::tts::TextToSpeech;
use crate::wake_word::{SimpleVAD, WakeWordDetector};
use cratos_core::{Orchestrator, OrchestratorInput};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Voice controller mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceMode {
    /// Full voice control (wake word + STT + TTS)
    Full,
    /// TTS only (no listening)
    TtsOnly,
    /// Push-to-talk (no wake word, manual trigger)
    PushToTalk,
}

/// Voice controller events
#[derive(Debug, Clone)]
pub enum VoiceEvent {
    /// Wake word detected
    WakeWordDetected,
    /// Started listening
    Listening,
    /// Stopped listening
    StoppedListening,
    /// Transcription result
    Transcribed(String),
    /// Speaking started
    Speaking,
    /// Speaking finished
    SpeakingFinished,
    /// Error occurred
    Error(String),
}

/// Unified voice controller
pub struct VoiceController {
    config: VoiceConfig,
    stt: SpeechToText,
    tts: TextToSpeech,
    mode: VoiceMode,
}

impl VoiceController {
    /// Create a new voice controller
    pub fn new(config: VoiceConfig) -> Result<Self> {
        let stt = SpeechToText::with_config(&config.language, &config.stt);
        let tts = TextToSpeech::new(&config.language);

        // Determine mode based on available features
        let mode = if stt.is_enabled() {
            info!("Voice control: Full mode (STT enabled)");
            VoiceMode::Full
        } else {
            info!("Voice control: TTS-only mode (OPENAI_API_KEY not set)");
            warn!("To enable voice recognition, set: export OPENAI_API_KEY=\"sk-...\"");
            VoiceMode::TtsOnly
        };

        info!("Assistant name: '{}'", config.wake_word.name);
        if !config.wake_word.alternatives.is_empty() {
            info!("Alternative triggers: {:?}", config.wake_word.alternatives);
        }

        Ok(Self {
            config,
            stt,
            tts,
            mode,
        })
    }

    /// Create with TTS-only mode (no listening)
    pub fn tts_only(config: VoiceConfig) -> Result<Self> {
        let tts = TextToSpeech::new(&config.language);
        let stt = SpeechToText::with_config(&config.language, &config.stt);

        Ok(Self {
            config,
            stt,
            tts,
            mode: VoiceMode::TtsOnly,
        })
    }

    /// Get current mode
    #[must_use]
    pub fn mode(&self) -> VoiceMode {
        self.mode
    }

    /// Check if STT is enabled
    #[must_use]
    pub fn stt_enabled(&self) -> bool {
        self.stt.is_enabled()
    }

    /// Speak text (TTS)
    pub async fn speak(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        info!("Speaking: {}...", &text[..text.len().min(50)]);

        let audio = self.tts.synthesize(text).await?;

        let output = AudioOutput::new()?;
        output.play_and_wait(&audio).await?;

        Ok(())
    }

    /// Synthesize text to audio without playing
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        self.tts.synthesize(text).await
    }

    /// Transcribe audio (STT)
    pub async fn transcribe(&self, audio_wav: &[u8]) -> Result<String> {
        self.stt.transcribe(audio_wav).await
    }

    /// Listen for voice input and transcribe
    pub async fn listen(&self) -> Result<String> {
        if !self.stt_enabled() {
            return Err(Error::NotEnabled("STT requires OPENAI_API_KEY".to_string()));
        }

        info!("Listening...");

        let mut input = AudioInput::new(self.config.sample_rate)?;

        // Play start beep
        let output = AudioOutput::new()?;
        let _ = output.play_beep().await;

        // Record until silence
        let samples = input
            .record_until_silence(
                self.config.threshold,
                self.config.silence_duration_ms,
                self.config.max_duration_secs,
            )
            .await?;

        if samples.is_empty() {
            return Err(Error::Stt("No audio captured".to_string()));
        }

        // Convert to WAV
        let wav = samples_to_wav(&samples, self.config.sample_rate)?;

        // Transcribe
        let text = self.stt.transcribe(&wav).await?;

        info!("Transcribed: {}", text);
        Ok(text)
    }

    /// Run interactive voice loop with orchestrator
    pub async fn run_interactive(
        &self,
        orchestrator: Arc<Orchestrator>,
        event_tx: Option<mpsc::Sender<VoiceEvent>>,
    ) -> Result<()> {
        if self.mode == VoiceMode::TtsOnly {
            warn!("TTS-only mode - interactive loop not available");
            warn!("Set OPENAI_API_KEY to enable voice recognition");
            return Ok(());
        }

        info!(
            "Starting voice loop. Say '{}' to activate.",
            self.config.wake_word.name
        );

        let mut input = AudioInput::new(self.config.sample_rate)?;
        let output = AudioOutput::new()?;

        // Try to load wake word detector, fall back to simple VAD
        let mut wake_detector = match WakeWordDetector::with_default_path(self.config.threshold) {
            Ok(detector) => Some(detector),
            Err(e) => {
                warn!("Wake word detector not available: {}", e);
                warn!("Using simple energy-based detection");
                None
            }
        };

        let simple_vad = SimpleVAD::new(self.config.threshold);
        let mut rx = input.start_recording()?;

        let mut waiting_for_command = false;
        let mut audio_buffer: Vec<f32> = Vec::new();

        loop {
            tokio::select! {
                samples = rx.recv() => {
                    let Some(samples) = samples else {
                        break;
                    };

                    if waiting_for_command {
                        // Accumulating audio for transcription
                        audio_buffer.extend(&samples);

                        // Check for silence (end of speech)
                        let _is_silent = if let Some(ref mut detector) = wake_detector {
                            !detector.is_voice_detected(&samples).unwrap_or(false)
                        } else {
                            !simple_vad.is_voice_detected(&samples)
                        };

                        let silence_threshold = (self.config.silence_duration_ms as f32 *
                            self.config.sample_rate as f32 / 1000.0) as usize;

                        // Check if we have enough silence at the end
                        if audio_buffer.len() > silence_threshold {
                            let tail = &audio_buffer[audio_buffer.len() - silence_threshold..];
                            let tail_energy = simple_vad.energy(tail);

                            if tail_energy < self.config.threshold {
                                // End of speech detected
                                waiting_for_command = false;

                                if let Some(tx) = &event_tx {
                                    let _ = tx.send(VoiceEvent::StoppedListening).await;
                                }

                                // Transcribe
                                let wav = samples_to_wav(&audio_buffer, self.config.sample_rate)?;
                                audio_buffer.clear();

                                match self.stt.transcribe(&wav).await {
                                    Ok(text) => {
                                        if text.is_empty() {
                                            continue;
                                        }

                                        if let Some(tx) = &event_tx {
                                            let _ = tx.send(VoiceEvent::Transcribed(text.clone())).await;
                                        }

                                        info!("User said: {}", text);

                                        // Process with orchestrator
                                        let orch_input = OrchestratorInput::new(
                                            "voice",
                                            "local",
                                            "voice_user",
                                            &text,
                                        );

                                        match orchestrator.process(orch_input).await {
                                            Ok(result) => {
                                                if let Some(tx) = &event_tx {
                                                    let _ = tx.send(VoiceEvent::Speaking).await;
                                                }

                                                let response = if result.response.is_empty() {
                                                    "완료했습니다.".to_string()
                                                } else {
                                                    result.response
                                                };

                                                // Speak response
                                                if let Ok(audio) = self.tts.synthesize(&response).await {
                                                    let _ = output.play_and_wait(&audio).await;
                                                }

                                                if let Some(tx) = &event_tx {
                                                    let _ = tx.send(VoiceEvent::SpeakingFinished).await;
                                                }
                                            }
                                            Err(e) => {
                                                error!("Orchestrator error: {}", e);
                                                let _ = self.speak("죄송합니다. 오류가 발생했습니다.").await;
                                            }
                                        }

                                        // Reset wake word detector
                                        if let Some(ref mut detector) = wake_detector {
                                            detector.reset();
                                        }
                                    }
                                    Err(e) => {
                                        error!("Transcription error: {}", e);
                                        if let Some(tx) = &event_tx {
                                            let _ = tx.send(VoiceEvent::Error(e.to_string())).await;
                                        }
                                    }
                                }
                            }
                        }

                        // Max duration check
                        let max_samples = self.config.max_duration_secs as usize *
                            self.config.sample_rate as usize;
                        if audio_buffer.len() > max_samples {
                            waiting_for_command = false;
                            audio_buffer.clear();
                            warn!("Max recording duration reached");
                        }
                    } else {
                        // Waiting for wake word (voice activity)
                        let is_voice = if let Some(ref mut detector) = wake_detector {
                            detector.is_voice_detected(&samples).unwrap_or(false)
                        } else {
                            simple_vad.is_voice_detected(&samples)
                        };

                        if is_voice {
                            info!("Voice activity detected - listening...");
                            waiting_for_command = true;
                            audio_buffer.clear();
                            audio_buffer.extend(&samples);

                            // Play beep
                            let _ = output.play_beep().await;

                            if let Some(tx) = &event_tx {
                                let _ = tx.send(VoiceEvent::WakeWordDetected).await;
                                let _ = tx.send(VoiceEvent::Listening).await;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get configuration
    #[must_use]
    pub fn config(&self) -> &VoiceConfig {
        &self.config
    }

    /// Get TTS instance
    #[must_use]
    pub fn tts(&self) -> &TextToSpeech {
        &self.tts
    }

    /// Get STT instance
    #[must_use]
    pub fn stt(&self) -> &SpeechToText {
        &self.stt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_controller_creation() {
        let config = VoiceConfig::default();
        let controller = VoiceController::new(config);
        assert!(controller.is_ok());
    }

    #[test]
    fn test_tts_only_mode() {
        let config = VoiceConfig::default();
        let controller = VoiceController::tts_only(config).unwrap();
        assert_eq!(controller.mode(), VoiceMode::TtsOnly);
    }
}
