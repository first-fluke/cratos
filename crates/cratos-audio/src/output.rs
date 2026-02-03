//! Audio output (speaker playback)

use crate::error::{Error, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Audio output for playing sounds
pub struct AudioOutput {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Arc<Mutex<Option<Sink>>>,
}

impl AudioOutput {
    /// Create a new audio output using the default output device
    pub fn new() -> Result<Self> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| Error::AudioDevice(format!("Failed to get output device: {}", e)))?;

        info!("Audio output initialized");

        Ok(Self {
            _stream: stream,
            handle,
            sink: Arc::new(Mutex::new(None)),
        })
    }

    /// Play audio from bytes (WAV, MP3, etc.)
    pub async fn play(&self, audio_data: &[u8]) -> Result<()> {
        let cursor = Cursor::new(audio_data.to_vec());

        let source = Decoder::new(cursor)
            .map_err(|e| Error::AudioStream(format!("Failed to decode audio: {}", e)))?;

        let sink = Sink::try_new(&self.handle)
            .map_err(|e| Error::AudioStream(format!("Failed to create sink: {}", e)))?;

        sink.append(source);

        // Store sink to keep playback alive
        {
            let mut sink_guard = self.sink.lock().await;
            *sink_guard = Some(sink);
        }

        debug!("Audio playback started");
        Ok(())
    }

    /// Play audio and wait for completion
    pub async fn play_and_wait(&self, audio_data: &[u8]) -> Result<()> {
        let cursor = Cursor::new(audio_data.to_vec());

        let source = Decoder::new(cursor)
            .map_err(|e| Error::AudioStream(format!("Failed to decode audio: {}", e)))?;

        let sink = Sink::try_new(&self.handle)
            .map_err(|e| Error::AudioStream(format!("Failed to create sink: {}", e)))?;

        sink.append(source);
        sink.sleep_until_end();

        debug!("Audio playback completed");
        Ok(())
    }

    /// Stop current playback
    pub async fn stop(&self) {
        let mut sink_guard = self.sink.lock().await;
        if let Some(sink) = sink_guard.take() {
            sink.stop();
        }
        debug!("Audio playback stopped");
    }

    /// Check if currently playing
    pub async fn is_playing(&self) -> bool {
        let sink_guard = self.sink.lock().await;
        sink_guard
            .as_ref()
            .map(|s| !s.empty())
            .unwrap_or(false)
    }

    /// Play a simple beep sound (for wake word detection feedback)
    pub async fn play_beep(&self) -> Result<()> {
        // Generate a simple 440Hz beep for 100ms
        let sample_rate = 44100u32;
        let duration_samples = sample_rate / 10; // 100ms
        let frequency = 440.0f32;

        let mut samples = Vec::with_capacity(duration_samples as usize);
        for i in 0..duration_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * frequency * t).sin();
            // Fade in/out to avoid clicks
            let envelope = if i < 1000 {
                i as f32 / 1000.0
            } else if i > duration_samples - 1000 {
                (duration_samples - i) as f32 / 1000.0
            } else {
                1.0
            };
            samples.push((sample * envelope * 0.3 * i16::MAX as f32) as i16);
        }

        // Create WAV in memory
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec)
                .map_err(|e| Error::AudioStream(format!("Failed to create WAV: {}", e)))?;

            for sample in samples {
                writer
                    .write_sample(sample)
                    .map_err(|e| Error::AudioStream(format!("Failed to write sample: {}", e)))?;
            }

            writer
                .finalize()
                .map_err(|e| Error::AudioStream(format!("Failed to finalize WAV: {}", e)))?;
        }

        self.play_and_wait(&cursor.into_inner()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require audio hardware and may not work in CI
    #[test]
    #[ignore = "Requires audio hardware"]
    fn test_audio_output_creation() {
        let output = AudioOutput::new();
        assert!(output.is_ok());
    }
}
