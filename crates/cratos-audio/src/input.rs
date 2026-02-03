//! Audio input (microphone capture)

use crate::error::{Error, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Audio sample type
pub type Sample = f32;

/// Audio input stream from microphone
pub struct AudioInput {
    device: Device,
    config: StreamConfig,
    stream: Option<Stream>,
    is_recording: Arc<AtomicBool>,
}

impl AudioInput {
    /// Create a new audio input from the default input device
    pub fn new(sample_rate: u32) -> Result<Self> {
        let host = cpal::default_host();

        let device = host
            .default_input_device()
            .ok_or_else(|| Error::AudioDevice("No input device found".to_string()))?;

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        info!("Using input device: {}", device_name);

        // Find a config that supports our sample rate
        let supported_configs = device
            .supported_input_configs()
            .map_err(|e| Error::AudioDevice(format!("Failed to get configs: {}", e)))?;

        let mut selected_config = None;
        for config in supported_configs {
            if config.min_sample_rate().0 <= sample_rate
                && config.max_sample_rate().0 >= sample_rate
                && config.sample_format() == SampleFormat::F32
            {
                selected_config = Some(config.with_sample_rate(cpal::SampleRate(sample_rate)));
                break;
            }
        }

        let supported = selected_config.ok_or_else(|| {
            Error::AudioDevice(format!("No config supports {}Hz F32", sample_rate))
        })?;

        let config: StreamConfig = supported.into();

        debug!(
            "Audio config: {} channels, {}Hz",
            config.channels, config.sample_rate.0
        );

        Ok(Self {
            device,
            config,
            stream: None,
            is_recording: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Get the sample rate
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }

    /// Get the number of channels
    #[must_use]
    pub fn channels(&self) -> u16 {
        self.config.channels
    }

    /// Check if currently recording
    #[must_use]
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Start recording and return a channel for receiving audio samples
    pub fn start_recording(&mut self) -> Result<mpsc::Receiver<Vec<Sample>>> {
        if self.is_recording() {
            return Err(Error::AudioStream("Already recording".to_string()));
        }

        let (tx, rx) = mpsc::channel::<Vec<Sample>>(100);
        let is_recording = self.is_recording.clone();
        let channels = self.config.channels as usize;

        let stream = self
            .device
            .build_input_stream(
                &self.config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !is_recording.load(Ordering::SeqCst) {
                        return;
                    }

                    // Convert to mono if stereo
                    let samples: Vec<f32> = if channels > 1 {
                        data.chunks(channels)
                            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                            .collect()
                    } else {
                        data.to_vec()
                    };

                    let _ = tx.try_send(samples);
                },
                move |err| {
                    error!("Audio input error: {}", err);
                },
                None,
            )
            .map_err(|e| Error::AudioStream(format!("Failed to build stream: {}", e)))?;

        stream
            .play()
            .map_err(|e| Error::AudioStream(format!("Failed to start stream: {}", e)))?;

        self.stream = Some(stream);
        self.is_recording.store(true, Ordering::SeqCst);

        info!("Audio recording started");
        Ok(rx)
    }

    /// Stop recording
    pub fn stop_recording(&mut self) {
        self.is_recording.store(false, Ordering::SeqCst);
        self.stream = None;
        info!("Audio recording stopped");
    }

    /// Record audio until silence is detected or max duration reached
    pub async fn record_until_silence(
        &mut self,
        silence_threshold: f32,
        silence_duration_ms: u64,
        max_duration_secs: u64,
    ) -> Result<Vec<Sample>> {
        let mut rx = self.start_recording()?;
        let sample_rate = self.sample_rate() as f64;

        let mut buffer = Vec::new();
        let mut silence_samples = 0u64;
        let silence_samples_threshold = (silence_duration_ms as f64 * sample_rate / 1000.0) as u64;
        let max_samples = (max_duration_secs as f64 * sample_rate) as u64;

        let timeout = tokio::time::Duration::from_secs(max_duration_secs + 1);
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => {
                    debug!("Max recording duration reached");
                    break;
                }
                samples = rx.recv() => {
                    let Some(samples) = samples else {
                        break;
                    };

                    // Check for silence
                    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();

                    if rms < silence_threshold {
                        silence_samples += samples.len() as u64;
                        if silence_samples > silence_samples_threshold && !buffer.is_empty() {
                            debug!("Silence detected, stopping recording");
                            break;
                        }
                    } else {
                        silence_samples = 0;
                    }

                    buffer.extend(samples);

                    if buffer.len() as u64 > max_samples {
                        debug!("Max samples reached");
                        break;
                    }
                }
            }
        }

        self.stop_recording();

        // Trim trailing silence
        let trim_samples = (silence_samples_threshold / 2) as usize;
        if buffer.len() > trim_samples {
            buffer.truncate(buffer.len() - trim_samples);
        }

        Ok(buffer)
    }
}

impl Drop for AudioInput {
    fn drop(&mut self) {
        self.stop_recording();
    }
}

/// Convert audio samples to WAV bytes
pub fn samples_to_wav(samples: &[Sample], sample_rate: u32) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| Error::AudioStream(format!("Failed to create WAV writer: {}", e)))?;

        for &sample in samples {
            let amplitude = (sample * i16::MAX as f32) as i16;
            writer
                .write_sample(amplitude)
                .map_err(|e| Error::AudioStream(format!("Failed to write sample: {}", e)))?;
        }

        writer
            .finalize()
            .map_err(|e| Error::AudioStream(format!("Failed to finalize WAV: {}", e)))?;
    }

    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_samples_to_wav() {
        let samples = vec![0.0f32; 1600]; // 0.1 second at 16kHz
        let wav = samples_to_wav(&samples, 16000).unwrap();

        // WAV header is 44 bytes
        assert!(wav.len() > 44);
        // Check RIFF header
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }
}
