//! Discord - serenity adapter
//!
//! This module provides the Discord bot adapter using the serenity library.

use crate::error::{Error, Result};
use crate::message::{ChannelAdapter, ChannelType, NormalizedMessage, OutgoingMessage};
use cratos_core::{Orchestrator, OrchestratorInput};
use serde::Deserialize;
use serenity::all::{
    ChannelId, Client, Context, CreateMessage, EditMessage, EventHandler, GatewayIntents,
    Message, MessageId, MessageReference, Ready,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};

/// Maximum length of text to log (to prevent sensitive data exposure)
const MAX_LOG_TEXT_LENGTH: usize = 50;

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

    for pattern in SENSITIVE_PATTERNS {
        if lower.contains(pattern) {
            return "[REDACTED - potentially sensitive content]".to_string();
        }
    }

    if text.len() > MAX_LOG_TEXT_LENGTH {
        format!("{}...[truncated]", &text[..MAX_LOG_TEXT_LENGTH])
    } else {
        text.to_string()
    }
}

/// Sanitize error messages to avoid exposing internal details
fn sanitize_error_for_user(error: &str) -> String {
    let lower = error.to_lowercase();

    if lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
    {
        return "An authentication error occurred. Please check your configuration.".to_string();
    }

    if lower.contains("connection") || lower.contains("timeout") || lower.contains("network") {
        return "A network error occurred. Please try again later.".to_string();
    }

    if lower.contains("database") || lower.contains("sql") || lower.contains("query") {
        return "A database error occurred. Please try again later.".to_string();
    }

    if error.len() > 100 || error.contains('/') || error.contains("at ") {
        return "An internal error occurred. Please try again.".to_string();
    }

    error.to_string()
}

/// Discord bot configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DiscordConfig {
    /// Bot token (from DISCORD_BOT_TOKEN env)
    pub bot_token: String,
    /// Allowed guild (server) IDs (empty = allow all)
    #[serde(default)]
    pub allowed_guilds: Vec<u64>,
    /// Allowed channel IDs (empty = allow all)
    #[serde(default)]
    pub allowed_channels: Vec<u64>,
    /// Whether to require @mention in guild channels
    #[serde(default = "default_true")]
    pub require_mention: bool,
}

fn default_true() -> bool {
    true
}

impl DiscordConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let bot_token = std::env::var("DISCORD_BOT_TOKEN")
            .map_err(|_| Error::Discord("DISCORD_BOT_TOKEN not set".to_string()))?;

        let allowed_guilds: Vec<u64> = std::env::var("DISCORD_ALLOWED_GUILDS")
            .ok()
            .map(|s| {
                s.split(',')
                    .filter_map(|id| id.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default();

        let allowed_channels: Vec<u64> = std::env::var("DISCORD_ALLOWED_CHANNELS")
            .ok()
            .map(|s| {
                s.split(',')
                    .filter_map(|id| id.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default();

        let require_mention = std::env::var("DISCORD_REQUIRE_MENTION")
            .map(|s| s != "false" && s != "0")
            .unwrap_or(true);

        Ok(Self {
            bot_token,
            allowed_guilds,
            allowed_channels,
            require_mention,
        })
    }

    /// Create with a bot token
    #[must_use]
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            allowed_guilds: Vec::new(),
            allowed_channels: Vec::new(),
            require_mention: true,
        }
    }

    /// Set allowed guilds
    #[must_use]
    pub fn with_allowed_guilds(mut self, guilds: Vec<u64>) -> Self {
        self.allowed_guilds = guilds;
        self
    }

    /// Set allowed channels
    #[must_use]
    pub fn with_allowed_channels(mut self, channels: Vec<u64>) -> Self {
        self.allowed_channels = channels;
        self
    }

    /// Set require mention mode
    #[must_use]
    pub fn with_require_mention(mut self, enabled: bool) -> Self {
        self.require_mention = enabled;
        self
    }
}

/// Discord bot adapter
pub struct DiscordAdapter {
    config: DiscordConfig,
    bot_user_id: AtomicU64,
    http: RwLock<Option<Arc<serenity::http::Http>>>,
}

impl DiscordAdapter {
    /// Create a new Discord adapter
    #[must_use]
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            config,
            bot_user_id: AtomicU64::new(0),
            http: RwLock::new(None),
        }
    }

    /// Create from environment
    pub fn from_env() -> Result<Self> {
        let config = DiscordConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Check if a guild is allowed
    pub fn is_guild_allowed(&self, guild_id: u64) -> bool {
        self.config.allowed_guilds.is_empty() || self.config.allowed_guilds.contains(&guild_id)
    }

    /// Check if a channel is allowed
    pub fn is_channel_allowed(&self, channel_id: u64) -> bool {
        self.config.allowed_channels.is_empty() || self.config.allowed_channels.contains(&channel_id)
    }

    /// Get the bot user ID
    pub fn bot_user_id(&self) -> u64 {
        self.bot_user_id.load(Ordering::SeqCst)
    }

    /// Start the bot with the given orchestrator
    #[instrument(skip(self, orchestrator))]
    pub async fn run(self: Arc<Self>, orchestrator: Arc<Orchestrator>) -> Result<()> {
        info!("Starting Discord bot");

        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

        let handler = DiscordHandler::new(self.clone(), orchestrator);

        let mut client = Client::builder(&self.config.bot_token, intents)
            .event_handler(handler)
            .await
            .map_err(|e| Error::Discord(format!("Failed to create client: {}", e)))?;

        // Store HTTP client for sending messages
        {
            let mut http_guard = self.http.write().await;
            *http_guard = Some(client.http.clone());
        }

        client
            .start()
            .await
            .map_err(|e| Error::Discord(format!("Client error: {}", e)))?;

        Ok(())
    }

    /// Convert a Discord message to a normalized message
    pub fn normalize_message(&self, msg: &Message) -> Option<NormalizedMessage> {
        let text = msg.content.clone();

        // Skip empty messages and bot messages
        if text.is_empty() || msg.author.bot {
            return None;
        }

        let channel_id = msg.channel_id.get();
        let user_id = msg.author.id.get();
        let guild_id = msg.guild_id.map(|g| g.get());

        // Check permissions
        if let Some(gid) = guild_id {
            if !self.is_guild_allowed(gid) {
                debug!(guild_id = %gid, "Guild not allowed");
                return None;
            }
        }

        if !self.is_channel_allowed(channel_id) {
            debug!(channel_id = %channel_id, "Channel not allowed");
            return None;
        }

        // In guild channels, check if we should respond
        if guild_id.is_some() && self.config.require_mention {
            let bot_id = self.bot_user_id();
            let is_mentioned = msg.mentions.iter().any(|u| u.id.get() == bot_id);
            let is_reply_to_bot = msg
                .referenced_message
                .as_ref()
                .map(|r| r.author.id.get() == bot_id)
                .unwrap_or(false);

            if !is_mentioned && !is_reply_to_bot {
                return None;
            }
        }

        let user_name = msg.author.name.clone();
        let message_id = msg.id.get().to_string();

        // Remove bot mention from text if present
        let bot_id = self.bot_user_id();
        let clean_text = text
            .replace(&format!("<@{}>", bot_id), "")
            .replace(&format!("<@!{}>", bot_id), "")
            .trim()
            .to_string();

        let mut normalized = NormalizedMessage::new(
            ChannelType::Discord,
            channel_id.to_string(),
            user_id.to_string(),
            message_id,
            clean_text,
        )
        .with_user_name(user_name);

        // Handle reply context
        if let Some(ref reply) = msg.referenced_message {
            normalized = normalized.with_thread(reply.id.get().to_string());
            normalized.is_reply = true;
        }

        Some(normalized)
    }
}

#[async_trait::async_trait]
impl ChannelAdapter for DiscordAdapter {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Discord
    }

    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String> {
        let channel_id: u64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid channel ID".to_string()))?;

        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| Error::Discord("Not connected".to_string()))?;

        let channel = ChannelId::new(channel_id);

        let mut builder = CreateMessage::new().content(&message.text);

        if let Some(reply_to) = &message.reply_to {
            if let Ok(msg_id) = reply_to.parse::<u64>() {
                builder = builder.reference_message(MessageReference::from((
                    ChannelId::new(channel_id),
                    MessageId::new(msg_id),
                )));
            }
        }

        let sent = channel
            .send_message(http, builder)
            .await
            .map_err(|e| Error::Discord(format!("Failed to send message: {}", e)))?;

        Ok(sent.id.get().to_string())
    }

    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        message: OutgoingMessage,
    ) -> Result<()> {
        let channel_id: u64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid channel ID".to_string()))?;
        let msg_id: u64 = message_id
            .parse()
            .map_err(|_| Error::Parse("Invalid message ID".to_string()))?;

        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| Error::Discord("Not connected".to_string()))?;

        let channel = ChannelId::new(channel_id);
        let builder = EditMessage::new().content(&message.text);

        channel
            .edit_message(http, MessageId::new(msg_id), builder)
            .await
            .map_err(|e| Error::Discord(format!("Failed to edit message: {}", e)))?;

        Ok(())
    }

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let channel_id: u64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid channel ID".to_string()))?;
        let msg_id: u64 = message_id
            .parse()
            .map_err(|_| Error::Parse("Invalid message ID".to_string()))?;

        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| Error::Discord("Not connected".to_string()))?;

        let channel = ChannelId::new(channel_id);

        channel
            .delete_message(http, MessageId::new(msg_id))
            .await
            .map_err(|e| Error::Discord(format!("Failed to delete message: {}", e)))?;

        Ok(())
    }

    async fn send_typing(&self, channel_id: &str) -> Result<()> {
        let channel_id: u64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid channel ID".to_string()))?;

        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| Error::Discord("Not connected".to_string()))?;

        let channel = ChannelId::new(channel_id);

        channel
            .broadcast_typing(http)
            .await
            .map_err(|e| Error::Discord(format!("Failed to send typing: {}", e)))?;

        Ok(())
    }
}

/// Discord event handler
struct DiscordHandler {
    adapter: Arc<DiscordAdapter>,
    orchestrator: Arc<Orchestrator>,
}

impl DiscordHandler {
    fn new(adapter: Arc<DiscordAdapter>, orchestrator: Arc<Orchestrator>) -> Self {
        Self {
            adapter,
            orchestrator,
        }
    }
}

#[serenity::async_trait]
impl EventHandler for DiscordHandler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        let discriminator = ready
            .user
            .discriminator
            .map(|d| format!("#{}", d))
            .unwrap_or_default();
        info!("Discord bot connected as {}{}", ready.user.name, discriminator);

        // Store bot user ID
        self.adapter
            .bot_user_id
            .store(ready.user.id.get(), Ordering::SeqCst);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let Some(normalized) = self.adapter.normalize_message(&msg) else {
            return;
        };

        // SECURITY: Mask potentially sensitive content in logs
        info!(
            channel_id = %normalized.channel_id,
            user_id = %normalized.user_id,
            text = %mask_for_logging(&normalized.text),
            "Received Discord message"
        );

        // Send typing indicator
        let _ = msg.channel_id.broadcast_typing(&ctx.http).await;

        // Process with orchestrator
        let input = OrchestratorInput::new(
            "discord",
            &normalized.channel_id,
            &normalized.user_id,
            &normalized.text,
        );

        match self.orchestrator.process(input).await {
            Ok(result) => {
                let response_text = if result.response.is_empty() {
                    "I've completed the task.".to_string()
                } else {
                    result.response
                };

                // Discord has a 2000 character limit
                let chunks: Vec<&str> = response_text
                    .as_bytes()
                    .chunks(2000)
                    .filter_map(|chunk| std::str::from_utf8(chunk).ok())
                    .collect();

                for chunk in chunks {
                    let builder = CreateMessage::new()
                        .content(chunk)
                        .reference_message(MessageReference::from((msg.channel_id, msg.id)));

                    if let Err(e) = msg.channel_id.send_message(&ctx.http, builder).await {
                        error!(error = %e, "Failed to send Discord response");
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to process Discord message");

                let user_message = sanitize_error_for_user(&e.to_string());
                let builder = CreateMessage::new()
                    .content(format!("Sorry, I encountered an error: {}", user_message))
                    .reference_message(MessageReference::from((msg.channel_id, msg.id)));

                let _ = msg.channel_id.send_message(&ctx.http, builder).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discord_config() {
        let config = DiscordConfig::new("test_token")
            .with_allowed_guilds(vec![123, 456])
            .with_require_mention(false);

        assert_eq!(config.bot_token, "test_token");
        assert_eq!(config.allowed_guilds, vec![123, 456]);
        assert!(!config.require_mention);
    }

    #[test]
    fn test_guild_allowed() {
        let config = DiscordConfig::new("token").with_allowed_guilds(vec![123, 456]);
        let adapter = DiscordAdapter::new(config);

        assert!(adapter.is_guild_allowed(123));
        assert!(adapter.is_guild_allowed(456));
        assert!(!adapter.is_guild_allowed(789));
    }

    #[test]
    fn test_empty_allowlist_allows_all() {
        let config = DiscordConfig::new("token");
        let adapter = DiscordAdapter::new(config);

        assert!(adapter.is_guild_allowed(123));
        assert!(adapter.is_guild_allowed(999999));
        assert!(adapter.is_channel_allowed(123));
    }

    #[test]
    fn test_mask_for_logging() {
        assert!(mask_for_logging("my password is secret123").contains("REDACTED"));
        assert!(mask_for_logging("API_KEY=sk-1234567890").contains("REDACTED"));

        let long_msg = "a".repeat(100);
        let masked = mask_for_logging(&long_msg);
        assert!(masked.contains("truncated"));

        assert_eq!(mask_for_logging("Hello, world!"), "Hello, world!");
    }

    #[test]
    fn test_sanitize_error_for_user() {
        let sanitized = sanitize_error_for_user("Invalid token: abc123");
        assert!(!sanitized.contains("abc123"));
        assert!(sanitized.contains("authentication"));

        let simple = sanitize_error_for_user("File not found");
        assert_eq!(simple, "File not found");
    }
}
