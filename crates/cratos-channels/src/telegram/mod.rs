//! Telegram - teloxide adapter
//!
//! This module provides the Telegram bot adapter using the teloxide library.

mod adapter;
#[cfg(feature = "audio")]
mod audio_utils;
mod channel_impl;
mod commands;
mod config;
mod handler;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use adapter::TelegramAdapter;
#[cfg(feature = "audio")]
pub use audio_utils::{convert_ogg_to_wav, is_ffmpeg_available};
pub use config::{DmPolicy, TelegramConfig};
