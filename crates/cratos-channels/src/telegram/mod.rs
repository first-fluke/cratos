//! Telegram - teloxide adapter
//!
//! This module provides the Telegram bot adapter using the teloxide library.

mod adapter;
mod channel_impl;
mod commands;
mod config;
mod handler;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use adapter::TelegramAdapter;
pub use config::{DmPolicy, TelegramConfig};
