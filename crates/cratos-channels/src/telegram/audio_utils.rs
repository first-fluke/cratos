//! Audio conversion utilities for Telegram voice messages
//!
//! Telegram sends voice messages as OGG/Opus format.
//! Whisper STT requires WAV format (16kHz mono).

use crate::error::{Error, Result};
use std::io::Write;
use std::process::{Command, Stdio};
use tracing::{debug, warn};

/// Convert OGG/Opus audio to WAV format suitable for Whisper STT
///
/// Uses ffmpeg for conversion (must be installed on the system).
///
/// # Arguments
/// * `ogg_data` - Raw OGG/Opus audio bytes
///
/// # Returns
/// WAV audio bytes (16kHz, mono, 16-bit)
///
/// # Errors
/// Returns error if ffmpeg is not available or conversion fails
pub fn convert_ogg_to_wav(ogg_data: &[u8]) -> Result<Vec<u8>> {
    debug!(input_size = ogg_data.len(), "Converting OGG to WAV");

    let mut child = Command::new("ffmpeg")
        .args([
            "-i", "pipe:0",         // Read from stdin
            "-f", "wav",            // Output format
            "-ar", "16000",         // Sample rate (Whisper expects 16kHz)
            "-ac", "1",             // Mono
            "-acodec", "pcm_s16le", // 16-bit PCM
            "pipe:1",               // Write to stdout
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            warn!(error = %e, "Failed to spawn ffmpeg");
            Error::Audio(format!(
                "ffmpeg not available. Install with: brew install ffmpeg. Error: {}",
                e
            ))
        })?;

    // Write input data
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(ogg_data).map_err(|e| {
            Error::Audio(format!("Failed to write to ffmpeg stdin: {}", e))
        })?;
    }

    // Read output
    let output = child.wait_with_output().map_err(|e| {
        Error::Audio(format!("ffmpeg execution failed: {}", e))
    })?;

    if !output.status.success() {
        return Err(Error::Audio(format!(
            "ffmpeg conversion failed with exit code: {:?}",
            output.status.code()
        )));
    }

    debug!(output_size = output.stdout.len(), "OGG to WAV conversion complete");
    Ok(output.stdout)
}

/// Check if ffmpeg is available on the system
#[must_use]
pub fn is_ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_check() {
        // This test just checks that the function doesn't panic
        let _available = is_ffmpeg_available();
    }
}
