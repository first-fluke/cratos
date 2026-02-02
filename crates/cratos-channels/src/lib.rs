//! Cratos Channels - Channel Adapters
//!
//! This crate provides channel adapters for different messaging platforms:
//! - Telegram (via teloxide)
//! - Slack (via slack-morphism)
//! - Discord (future, via serenity)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod message;
pub mod slack;
pub mod telegram;

pub use error::{Error, Result};

// Re-export message types
pub use message::{
    Attachment, AttachmentType, ChannelAdapter, ChannelType, MessageButton, NormalizedMessage,
    OutgoingMessage,
};

// Re-export Telegram adapter
pub use telegram::{TelegramAdapter, TelegramConfig};

// Re-export Slack adapter
pub use slack::{SlackAdapter, SlackConfig};
