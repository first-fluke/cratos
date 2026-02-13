//! Telegram adapter core

use super::config::{DmPolicy, TelegramConfig};
use crate::error::Result;
use crate::message::{
    Attachment, AttachmentType, ChannelType, MessageButton, NormalizedMessage,
};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, Message as TelegramMessage},
};
use tracing::debug;

/// Telegram bot adapter
pub struct TelegramAdapter {
    pub(crate) bot: Bot,
    pub(crate) config: TelegramConfig,
}

impl TelegramAdapter {
    /// Create a new Telegram adapter
    #[must_use]
    pub fn new(config: TelegramConfig) -> Self {
        let bot = Bot::new(&config.bot_token);
        Self { bot, config }
    }

    /// Create from environment
    pub fn from_env() -> Result<Self> {
        let config = TelegramConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Get the underlying bot
    pub fn bot(&self) -> &Bot {
        &self.bot
    }

    /// Check if a user is allowed
    pub fn is_user_allowed(&self, user_id: i64) -> bool {
        self.config.allowed_users.is_empty() || self.config.allowed_users.contains(&user_id)
    }

    /// Check if a group is allowed
    pub fn is_group_allowed(&self, chat_id: i64) -> bool {
        self.config.allowed_groups.is_empty() || self.config.allowed_groups.contains(&chat_id)
    }

    /// Convert a Telegram message to a normalized message
    pub fn normalize_message(
        &self,
        msg: &TelegramMessage,
        bot_username: &str,
    ) -> Option<NormalizedMessage> {
        // For photo/document messages, text() is None — use caption() instead
        let text = msg
            .text()
            .or_else(|| msg.caption())
            .unwrap_or("")
            .to_string();

        // Skip messages with no text AND no photo (pure empty)
        let has_photo = msg.photo().is_some();
        if text.is_empty() && !has_photo {
            return None;
        }

        let user = msg.from.as_ref()?;
        let chat_id = msg.chat.id.0;
        let user_id = user.id.0;

        // Check permissions
        let is_dm = msg.chat.is_private();

        if is_dm {
            match self.config.dm_policy {
                DmPolicy::Disabled => {
                    debug!(user_id = %user_id, "DMs are disabled");
                    return None;
                }
                DmPolicy::Allowlist => {
                    if !self.is_user_allowed(user_id as i64) {
                        debug!(user_id = %user_id, "User not in allowlist");
                        return None;
                    }
                }
                DmPolicy::Pairing => {
                    // Pairing mode: allow listed users, block unknown
                    // Full pairing code flow would require state storage
                    if !self.is_user_allowed(user_id as i64) {
                        debug!(user_id = %user_id, "User not paired");
                        return None;
                    }
                }
                DmPolicy::Open => {
                    // Accept all DMs
                }
            }
        } else if !self.is_user_allowed(user_id as i64) {
            debug!(user_id = %user_id, "User not allowed");
            return None;
        }

        if msg.chat.is_group() || msg.chat.is_supergroup() {
            if !self.is_group_allowed(chat_id) {
                debug!(chat_id = %chat_id, "Group not allowed");
                return None;
            }

            // In groups, check if we should respond
            if self.config.groups_mention_only {
                let is_reply_to_bot = msg
                    .reply_to_message()
                    .and_then(|r| r.from.as_ref())
                    .map(|u| u.username.as_deref() == Some(bot_username))
                    .unwrap_or(false);

                let is_mention = text.contains(&format!("@{}", bot_username));

                if !is_reply_to_bot && !is_mention {
                    return None;
                }
            }
        }

        let user_name = user.first_name.clone();
        let message_id = msg.id.0.to_string();

        let mut normalized = NormalizedMessage::new(
            ChannelType::Telegram,
            chat_id.to_string(),
            user_id.to_string(),
            message_id,
            text,
        )
        .with_user_name(user_name);

        // Handle reply context
        if let Some(reply) = msg.reply_to_message() {
            normalized = normalized.with_thread(reply.id.0.to_string());
            normalized.is_reply = true;
        }

        // Handle attachments
        if let Some(photo) = msg.photo() {
            if let Some(largest) = photo.last() {
                normalized = normalized.with_attachment(Attachment {
                    attachment_type: AttachmentType::Image,
                    file_name: None,
                    mime_type: Some("image/jpeg".to_string()),
                    file_size: Some(largest.file.size as u64),
                    url: None,
                    file_id: Some(largest.file.id.to_string()),
                });
            }
        }

        if let Some(doc) = msg.document() {
            normalized = normalized.with_attachment(Attachment {
                attachment_type: AttachmentType::Document,
                file_name: doc.file_name.clone(),
                mime_type: doc.mime_type.as_ref().map(|m| m.to_string()),
                file_size: Some(doc.file.size as u64),
                url: None,
                file_id: Some(doc.file.id.to_string()),
            });
        }

        Some(normalized)
    }

    /// Build inline keyboard from buttons
    pub fn build_keyboard(buttons: &[MessageButton]) -> Option<InlineKeyboardMarkup> {
        if buttons.is_empty() {
            return None;
        }

        let keyboard_buttons: Vec<InlineKeyboardButton> = buttons
            .iter()
            .filter_map(|b| {
                if let Some(callback_data) = &b.callback_data {
                    Some(InlineKeyboardButton::callback(&b.text, callback_data))
                } else if let Some(url) = &b.url {
                    Some(InlineKeyboardButton::url(&b.text, url.parse().ok()?))
                } else {
                    None
                }
            })
            .collect();

        // Single row for simplicity
        Some(InlineKeyboardMarkup::new(vec![keyboard_buttons]))
    }
}
