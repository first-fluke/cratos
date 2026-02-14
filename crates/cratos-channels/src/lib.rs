//! Cratos Channels - Channel Adapters
//!
//! This crate provides channel adapters for different messaging platforms:
//! - Telegram (via teloxide)
//! - Slack (via slack-morphism)
//! - Discord (via serenity)
//! - WhatsApp (via Baileys bridge or Business API)
//! - Matrix (via matrix-sdk)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod discord;
pub mod error;
pub mod matrix;
pub mod message;
pub mod slack;
pub mod telegram;
pub mod util;
pub mod whatsapp;
pub mod whatsapp_business;

pub use error::{Error, Result};

// Re-export message types
pub use message::{
    Attachment, AttachmentType, ChannelAdapter, ChannelType, MessageButton, NormalizedMessage,
    OutgoingAttachment, OutgoingMessage,
};

// Re-export Telegram adapter
pub use telegram::{TelegramAdapter, TelegramConfig};

// Re-export Slack adapter
pub use slack::{SlackAdapter, SlackConfig};

// Re-export Discord adapter
pub use discord::{DiscordAdapter, DiscordConfig};

// Re-export WhatsApp adapters
pub use whatsapp::{WhatsAppAdapter, WhatsAppConfig};
pub use whatsapp_business::{WhatsAppBusinessAdapter, WhatsAppBusinessConfig, WhatsAppBusinessWebhook};

// Re-export Matrix adapter
pub use matrix::{MatrixAdapter, MatrixConfig};
