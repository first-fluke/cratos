//! Telegram - teloxide adapter
//!
//! This module provides the Telegram bot adapter using the teloxide library.

use crate::error::{Error, Result};
use crate::message::{
    Attachment, AttachmentType, ChannelAdapter, ChannelType, MessageButton, NormalizedMessage,
    OutgoingMessage,
};
use cratos_core::{Orchestrator, OrchestratorInput};
use std::sync::Arc;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    types::{
        ChatAction, InlineKeyboardButton, InlineKeyboardMarkup, Message as TelegramMessage,
        MessageId, ParseMode,
    },
};
use tracing::{debug, error, info, instrument};

/// Maximum length of text to log (to prevent sensitive data exposure)
const MAX_LOG_TEXT_LENGTH: usize = 50;

/// Maximum length of error message to show to users (longer = likely internal)
const MAX_SAFE_ERROR_LENGTH: usize = 100;

/// Patterns that indicate potentially sensitive content
const SENSITIVE_PATTERNS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "bearer",
    "authorization",
    "credential",
    "private",
    "ssh",
    "-----begin",
];

/// Mask potentially sensitive text for logging
fn mask_for_logging(text: &str) -> String {
    let lower = text.to_lowercase();

    // Check for sensitive patterns
    for pattern in SENSITIVE_PATTERNS {
        if lower.contains(pattern) {
            return "[REDACTED - potentially sensitive content]".to_string();
        }
    }

    // Truncate long messages
    if text.len() > MAX_LOG_TEXT_LENGTH {
        format!("{}...[truncated]", &text[..MAX_LOG_TEXT_LENGTH])
    } else {
        text.to_string()
    }
}

/// Sanitize error messages to avoid exposing internal details
fn sanitize_error_for_user(error: &str) -> String {
    // Don't expose internal paths, stack traces, or sensitive info
    let lower = error.to_lowercase();

    if lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
    {
        return "An authentication error occurred. Please check your configuration.".to_string();
    }

    if lower.contains("connection")
        || lower.contains("timeout")
        || lower.contains("network")
    {
        return "A network error occurred. Please try again later.".to_string();
    }

    if lower.contains("database") || lower.contains("sql") || lower.contains("query") {
        return "A database error occurred. Please try again later.".to_string();
    }

    // For other errors, give a generic message
    if error.len() > MAX_SAFE_ERROR_LENGTH || error.contains('/') || error.contains("at ") {
        return "An internal error occurred. Please try again.".to_string();
    }

    // Short, non-sensitive errors can be shown
    error.to_string()
}

/// Telegram bot configuration
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    /// Bot token
    pub bot_token: String,
    /// Allowed user IDs (empty = allow all)
    pub allowed_users: Vec<i64>,
    /// Allowed group IDs (empty = allow all)
    pub allowed_groups: Vec<i64>,
    /// Whether to respond only to mentions/replies in groups
    pub groups_mention_only: bool,
}

impl TelegramConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| Error::Telegram("TELEGRAM_BOT_TOKEN not set".to_string()))?;

        let allowed_users: Vec<i64> = std::env::var("TELEGRAM_ALLOWED_USERS")
            .ok()
            .map(|s| {
                s.split(',')
                    .filter_map(|id| id.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default();

        let allowed_groups: Vec<i64> = std::env::var("TELEGRAM_ALLOWED_GROUPS")
            .ok()
            .map(|s| {
                s.split(',')
                    .filter_map(|id| id.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default();

        let groups_mention_only = std::env::var("TELEGRAM_GROUPS_MENTION_ONLY")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(true);

        Ok(Self {
            bot_token,
            allowed_users,
            allowed_groups,
            groups_mention_only,
        })
    }

    /// Create with a bot token
    #[must_use]
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            allowed_users: Vec::new(),
            allowed_groups: Vec::new(),
            groups_mention_only: true,
        }
    }

    /// Set allowed users
    #[must_use]
    pub fn with_allowed_users(mut self, users: Vec<i64>) -> Self {
        self.allowed_users = users;
        self
    }

    /// Set allowed groups
    #[must_use]
    pub fn with_allowed_groups(mut self, groups: Vec<i64>) -> Self {
        self.allowed_groups = groups;
        self
    }

    /// Set groups mention only mode
    #[must_use]
    pub fn with_groups_mention_only(mut self, enabled: bool) -> Self {
        self.groups_mention_only = enabled;
        self
    }
}

/// Telegram bot adapter
pub struct TelegramAdapter {
    bot: Bot,
    config: TelegramConfig,
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
        let text = msg.text().unwrap_or("").to_string();

        // Skip empty messages
        if text.is_empty() {
            return None;
        }

        let user = msg.from()?;
        let chat_id = msg.chat.id.0;
        let user_id = user.id.0;

        // Check permissions
        if !self.is_user_allowed(user_id as i64) {
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
                    .and_then(|r| r.from())
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
                    file_id: Some(largest.file.id.clone()),
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
                file_id: Some(doc.file.id.clone()),
            });
        }

        Some(normalized)
    }

    /// Build inline keyboard from buttons
    fn build_keyboard(buttons: &[MessageButton]) -> Option<InlineKeyboardMarkup> {
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

    /// Start the bot with the given orchestrator
    #[instrument(skip(self, orchestrator))]
    pub async fn run(self: Arc<Self>, orchestrator: Arc<Orchestrator>) -> Result<()> {
        info!("Starting Telegram bot");

        let bot = self.bot.clone();
        let adapter = self.clone();

        let handler = Update::filter_message().endpoint(move |bot: Bot, msg: TelegramMessage| {
            let adapter = adapter.clone();
            let orchestrator = orchestrator.clone();
            async move { Self::handle_message(adapter, orchestrator, bot, msg).await }
        });

        Dispatcher::builder(bot, handler)
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;

        Ok(())
    }

    /// Handle an incoming message
    async fn handle_message(
        adapter: Arc<Self>,
        orchestrator: Arc<Orchestrator>,
        bot: Bot,
        msg: TelegramMessage,
    ) -> ResponseResult<()> {
        let bot_username = bot
            .get_me()
            .await
            .map(|me| me.username.clone().unwrap_or_default())
            .unwrap_or_default();

        let Some(normalized) = adapter.normalize_message(&msg, &bot_username) else {
            return Ok(());
        };

        // SECURITY: Mask potentially sensitive content in logs
        info!(
            chat_id = %normalized.channel_id,
            user_id = %normalized.user_id,
            text = %mask_for_logging(&normalized.text),
            "Received message"
        );

        // Send typing indicator
        let _ = bot.send_chat_action(msg.chat.id, ChatAction::Typing).await;

        // Process with orchestrator
        let input = OrchestratorInput::new(
            "telegram",
            &normalized.channel_id,
            &normalized.user_id,
            &normalized.text,
        );

        match orchestrator.process(input).await {
            Ok(result) => {
                let response_text = if result.response.is_empty() {
                    "I've completed the task.".to_string()
                } else {
                    result.response
                };

                // Send response
                let send_result = bot
                    .send_message(msg.chat.id, &response_text)
                    .parse_mode(ParseMode::MarkdownV2)
                    .reply_to_message_id(msg.id)
                    .await;

                // Fall back to plain text if markdown fails
                if send_result.is_err() {
                    let _ = bot
                        .send_message(msg.chat.id, &response_text)
                        .reply_to_message_id(msg.id)
                        .await;
                }
            }
            Err(e) => {
                // Log full error internally
                error!(error = %e, "Failed to process message");

                // SECURITY: Send sanitized error to user (don't expose internal details)
                let user_message = sanitize_error_for_user(&e.to_string());
                let _ = bot
                    .send_message(msg.chat.id, format!("Sorry, I encountered an error: {}", user_message))
                    .reply_to_message_id(msg.id)
                    .await;
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ChannelAdapter for TelegramAdapter {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String> {
        let chat_id: i64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid chat ID".to_string()))?;

        let mut request = self.bot.send_message(ChatId(chat_id), &message.text);

        if message.parse_markdown {
            request = request.parse_mode(ParseMode::MarkdownV2);
        }

        if let Some(reply_to) = &message.reply_to {
            if let Ok(msg_id) = reply_to.parse::<i32>() {
                request = request.reply_to_message_id(MessageId(msg_id));
            }
        }

        if let Some(keyboard) = Self::build_keyboard(&message.buttons) {
            request = request.reply_markup(keyboard);
        }

        let sent = request.await.map_err(|e| Error::Telegram(e.to_string()))?;

        Ok(sent.id.0.to_string())
    }

    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        message: OutgoingMessage,
    ) -> Result<()> {
        let chat_id: i64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid chat ID".to_string()))?;
        let msg_id: i32 = message_id
            .parse()
            .map_err(|_| Error::Parse("Invalid message ID".to_string()))?;

        let mut request =
            self.bot
                .edit_message_text(ChatId(chat_id), MessageId(msg_id), &message.text);

        if message.parse_markdown {
            request = request.parse_mode(ParseMode::MarkdownV2);
        }

        request.await.map_err(|e| Error::Telegram(e.to_string()))?;

        Ok(())
    }

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let chat_id: i64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid chat ID".to_string()))?;
        let msg_id: i32 = message_id
            .parse()
            .map_err(|_| Error::Parse("Invalid message ID".to_string()))?;

        self.bot
            .delete_message(ChatId(chat_id), MessageId(msg_id))
            .await
            .map_err(|e| Error::Telegram(e.to_string()))?;

        Ok(())
    }

    async fn send_typing(&self, channel_id: &str) -> Result<()> {
        let chat_id: i64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid chat ID".to_string()))?;

        self.bot
            .send_chat_action(ChatId(chat_id), ChatAction::Typing)
            .await
            .map_err(|e| Error::Telegram(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_config() {
        let config = TelegramConfig::new("test_token")
            .with_allowed_users(vec![123, 456])
            .with_groups_mention_only(false);

        assert_eq!(config.bot_token, "test_token");
        assert_eq!(config.allowed_users, vec![123, 456]);
        assert!(!config.groups_mention_only);
    }

    #[test]
    fn test_build_keyboard() {
        let buttons = vec![
            MessageButton::callback("Yes", "approve:yes"),
            MessageButton::callback("No", "approve:no"),
        ];

        let keyboard = TelegramAdapter::build_keyboard(&buttons);
        assert!(keyboard.is_some());
    }

    #[test]
    fn test_user_allowed() {
        let config = TelegramConfig::new("token").with_allowed_users(vec![123, 456]);
        let adapter = TelegramAdapter::new(config);

        assert!(adapter.is_user_allowed(123));
        assert!(adapter.is_user_allowed(456));
        assert!(!adapter.is_user_allowed(789));
    }

    #[test]
    fn test_empty_allowlist_allows_all() {
        let config = TelegramConfig::new("token");
        let adapter = TelegramAdapter::new(config);

        assert!(adapter.is_user_allowed(123));
        assert!(adapter.is_user_allowed(999999));
    }

    #[test]
    fn test_mask_for_logging() {
        // Should mask sensitive content
        assert!(mask_for_logging("my password is secret123").contains("REDACTED"));
        assert!(mask_for_logging("API_KEY=sk-1234567890").contains("REDACTED"));
        assert!(mask_for_logging("Bearer eyJhbGciOiJ").contains("REDACTED"));
        assert!(mask_for_logging("-----BEGIN RSA PRIVATE KEY-----").contains("REDACTED"));

        // Should truncate long messages
        let long_msg = "a".repeat(100);
        let masked = mask_for_logging(&long_msg);
        assert!(masked.contains("truncated"));
        assert!(masked.len() < long_msg.len());

        // Should pass through normal short messages
        assert_eq!(mask_for_logging("Hello, world!"), "Hello, world!");
        assert_eq!(mask_for_logging("요약해줘"), "요약해줘");
    }

    #[test]
    fn test_sanitize_error_for_user() {
        // Should hide token/auth errors
        let sanitized = sanitize_error_for_user("Invalid token: abc123");
        assert!(!sanitized.contains("abc123"));
        assert!(sanitized.contains("authentication"));

        // Should hide database errors
        let sanitized = sanitize_error_for_user("SQL error: SELECT * FROM users");
        assert!(!sanitized.contains("SELECT"));
        assert!(sanitized.contains("database"));

        // Should hide paths and stack traces
        // Note: using "config.json" instead of "secret.json" to avoid triggering auth error detection
        let sanitized = sanitize_error_for_user(
            "Error at /home/user/.config/app/config.json line 42"
        );
        assert!(!sanitized.contains("/home"));
        assert!(
            sanitized.to_lowercase().contains("internal"),
            "Expected 'internal' in sanitized error, got: {}", sanitized
        );

        // Should pass through simple, safe errors
        let simple = sanitize_error_for_user("File not found");
        assert_eq!(simple, "File not found");
    }
}
