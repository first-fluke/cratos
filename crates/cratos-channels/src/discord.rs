//! Discord - serenity adapter
//!
//! This module provides the Discord bot adapter using the serenity library.

use crate::error::{Error, Result};
use crate::message::{ChannelAdapter, ChannelType, NormalizedMessage, OutgoingMessage};
use crate::util::{mask_for_logging, sanitize_error_for_user, DISCORD_MESSAGE_LIMIT};
use cratos_core::event_bus::OrchestratorEvent;
use cratos_core::{Orchestrator, OrchestratorInput};
use serde::Deserialize;
use serenity::all::{
    ButtonStyle, ChannelId, Client, Command, CommandInteraction, CommandOptionType,
    ComponentInteraction, Context, CreateActionRow, CreateButton, CreateCommand,
    CreateCommandOption, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, EditMessage, EventHandler,
    GatewayIntents, Interaction, Message, MessageId, MessageReference, Ready,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

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
    /// DM policy: "open" (default) | "disabled"
    #[serde(default = "default_dm_open")]
    pub dm_policy: String,
    /// Notification channel ID for EventBus alerts (approval requests, failures)
    #[serde(default)]
    pub notify_channel_id: Option<u64>,
}

fn default_dm_open() -> String {
    "open".to_string()
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

        let dm_policy = std::env::var("DISCORD_DM_POLICY").unwrap_or_else(|_| "open".to_string());

        let notify_channel_id: Option<u64> = std::env::var("DISCORD_NOTIFY_CHANNEL_ID")
            .ok()
            .and_then(|s| s.trim().parse().ok());

        Ok(Self {
            bot_token,
            allowed_guilds,
            allowed_channels,
            require_mention,
            dm_policy,
            notify_channel_id,
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
            dm_policy: "open".to_string(),
            notify_channel_id: None,
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
        self.config.allowed_channels.is_empty()
            || self.config.allowed_channels.contains(&channel_id)
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

        let handler = DiscordHandler::new(self.clone(), orchestrator.clone());

        let mut client = Client::builder(&self.config.bot_token, intents)
            .event_handler(handler)
            .await
            .map_err(|e| Error::Discord(format!("Failed to create client: {}", e)))?;

        // Store HTTP client for sending messages
        {
            let mut http_guard = self.http.write().await;
            *http_guard = Some(client.http.clone());
        }

        // Spawn EventBus listener for approval/failure notifications
        if let (Some(notify_ch), Some(bus)) = (
            self.config.notify_channel_id,
            orchestrator.event_bus().cloned(),
        ) {
            let http_for_events = client.http.clone();
            let channel_id = ChannelId::new(notify_ch);
            let mut rx = bus.subscribe();
            let orch_for_events = orchestrator.clone();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(OrchestratorEvent::ApprovalRequired {
                            execution_id,
                            request_id,
                        }) => {
                            let embed = CreateEmbed::new()
                                .title("Approval Required")
                                .description(format!("Execution `{}` requires approval.", execution_id))
                                .field("Request ID", request_id.to_string(), true)
                                .color(0xffaa00);
                            let approve_btn = CreateButton::new(format!("approve:{}", request_id))
                                .label("Approve")
                                .style(ButtonStyle::Success);
                            let deny_btn = CreateButton::new(format!("deny:{}", request_id))
                                .label("Deny")
                                .style(ButtonStyle::Danger);
                            let row = CreateActionRow::Buttons(vec![approve_btn, deny_btn]);
                            let msg = CreateMessage::new()
                                .embed(embed)
                                .components(vec![row]);
                            if let Err(e) = channel_id.send_message(&http_for_events, msg).await {
                                warn!(error = %e, "Failed to send approval notification to Discord");
                            }
                        }
                        Ok(OrchestratorEvent::ExecutionFailed {
                            execution_id,
                            error,
                        }) => {
                            let embed = CreateEmbed::new()
                                .title("Execution Failed")
                                .description(format!("Execution `{}` failed.", execution_id))
                                .field("Error", &error, false)
                                .color(0xff0000);
                            let msg = CreateMessage::new().embed(embed);
                            if let Err(e) = channel_id.send_message(&http_for_events, msg).await {
                                warn!(error = %e, "Failed to send failure notification to Discord");
                            }
                        }
                        Ok(_) => {} // Ignore other events
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Discord EventBus listener lagged by {} events", n);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            info!("Discord EventBus channel closed, stopping listener");
                            break;
                        }
                    }
                }
                let _ = orch_for_events; // keep orchestrator alive
            });
            info!("Discord EventBus notification listener started (channel: {})", notify_ch);
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

        // DM policy check: if guild_id is None, the message is a DM
        let is_dm = guild_id.is_none();
        if is_dm && self.config.dm_policy == "disabled" {
            debug!(user_id = %user_id, "DM rejected by dm_policy=disabled");
            return None;
        }

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
    async fn ready(&self, ctx: Context, ready: Ready) {
        let discriminator = ready
            .user
            .discriminator
            .map(|d| format!("#{}", d))
            .unwrap_or_default();
        info!(
            "Discord bot connected as {}{}",
            ready.user.name, discriminator
        );

        // Store bot user ID
        self.adapter
            .bot_user_id
            .store(ready.user.id.get(), Ordering::SeqCst);

        // Register global slash commands
        let commands = vec![
            CreateCommand::new("status").description("Show system status"),
            CreateCommand::new("sessions").description("List active AI sessions"),
            CreateCommand::new("tools").description("List available tools"),
            CreateCommand::new("cancel")
                .description("Cancel an execution")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "id",
                        "Execution ID to cancel",
                    )
                    .required(true),
                ),
            CreateCommand::new("approve")
                .description("Approve a pending request")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "id",
                        "Request ID to approve",
                    )
                    .required(true),
                ),
        ];

        match Command::set_global_commands(&ctx.http, commands).await {
            Ok(cmds) => info!("Registered {} Discord slash commands", cmds.len()),
            Err(e) => error!(error = %e, "Failed to register Discord slash commands"),
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                let response = match command.data.name.as_str() {
                    "status" => {
                        let embed = self.status_embed();
                        CreateInteractionResponseMessage::new().embed(embed)
                    }
                    "sessions" => {
                        CreateInteractionResponseMessage::new().content(self.handle_sessions())
                    }
                    "tools" => {
                        CreateInteractionResponseMessage::new().content(self.handle_tools())
                    }
                    "cancel" => {
                        let id = get_string_option(&command, "id").unwrap_or_default();
                        CreateInteractionResponseMessage::new().content(self.handle_cancel(&id))
                    }
                    "approve" => {
                        let id = get_string_option(&command, "id").unwrap_or_default();
                        CreateInteractionResponseMessage::new().content(self.handle_approve(&id))
                    }
                    _ => CreateInteractionResponseMessage::new().content("Unknown command"),
                };

                let builder = CreateInteractionResponse::Message(response);
                if let Err(e) = command.create_response(&ctx.http, builder).await {
                    error!(error = %e, "Failed to respond to slash command");
                }
            }
            Interaction::Component(component) => {
                self.handle_component(&ctx, &component).await;
            }
            _ => {}
        }
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

                // Discord has a message character limit
                let chunks: Vec<&str> = response_text
                    .as_bytes()
                    .chunks(DISCORD_MESSAGE_LIMIT)
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

impl DiscordHandler {
    /// Build a rich embed for the /status command
    fn status_embed(&self) -> CreateEmbed {
        let count = self.orchestrator.active_execution_count().unwrap_or(0);
        let provider = self.orchestrator.provider_name().to_string();
        let color = if count == 0 { 0x00ff00 } else { 0xffaa00 };
        CreateEmbed::new()
            .title("Cratos Status")
            .field("Active Executions", count.to_string(), true)
            .field("Provider", provider, true)
            .color(color)
    }

    /// Handle button (component) interactions from approval/deny embeds
    async fn handle_component(&self, ctx: &Context, component: &ComponentInteraction) {
        let custom_id = &component.data.custom_id;
        let response_text = if let Some(id) = custom_id.strip_prefix("approve:") {
            self.handle_approve(id)
        } else if let Some(id) = custom_id.strip_prefix("deny:") {
            format!("Request `{}` denied.", id)
        } else {
            "Unknown action.".to_string()
        };

        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(response_text)
                .ephemeral(true),
        );
        if let Err(e) = component.create_response(&ctx.http, builder).await {
            error!(error = %e, "Failed to respond to component interaction");
        }
    }

    fn handle_sessions(&self) -> String {
        let count = self
            .orchestrator
            .active_execution_count()
            .unwrap_or(0);
        if count == 0 {
            "No active sessions.".to_string()
        } else {
            format!("{} active session(s) running.", count)
        }
    }

    fn handle_tools(&self) -> String {
        let tools = self.orchestrator.list_tool_names();
        if tools.is_empty() {
            "No tools available.".to_string()
        } else {
            format!(
                "**Available tools ({}):**\n{}",
                tools.len(),
                tools.join(", ")
            )
        }
    }

    fn handle_cancel(&self, id: &str) -> String {
        if id.is_empty() {
            return "Please provide an execution ID.".to_string();
        }
        match uuid::Uuid::parse_str(id) {
            Ok(execution_id) => {
                if self.orchestrator.cancel_execution(execution_id) {
                    format!("Execution `{}` cancelled.", id)
                } else {
                    format!("Execution `{}` not found or already completed.", id)
                }
            }
            Err(_) => "Invalid execution ID format.".to_string(),
        }
    }

    fn handle_approve(&self, id: &str) -> String {
        if id.is_empty() {
            return "Please provide a request ID.".to_string();
        }
        let request_id = match uuid::Uuid::parse_str(id) {
            Ok(uid) => uid,
            Err(_) => return "Invalid request ID format.".to_string(),
        };
        match self.orchestrator.approval_manager() {
            Some(mgr) => {
                let mgr_clone = mgr.clone();
                tokio::spawn(async move {
                    mgr_clone.approve_by(request_id, "discord").await;
                });
                format!("Approval request `{}` processed.", id)
            }
            None => "Approval manager not configured.".to_string(),
        }
    }
}

/// Extract a string option from a slash command interaction
fn get_string_option(command: &CommandInteraction, name: &str) -> Option<String> {
    command
        .data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_str().map(|s| s.to_string()))
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
    fn test_dm_policy_default_open() {
        let config = DiscordConfig::new("token");
        assert_eq!(config.dm_policy, "open");
        assert!(config.notify_channel_id.is_none());
    }

    #[test]
    fn test_component_custom_id_parsing() {
        let approve_id = "approve:550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(
            approve_id.strip_prefix("approve:"),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );

        let deny_id = "deny:550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(
            deny_id.strip_prefix("deny:"),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );

        let unknown = "unknown:something";
        assert!(unknown.strip_prefix("approve:").is_none());
        assert!(unknown.strip_prefix("deny:").is_none());
    }

    // Note: mask_for_logging and sanitize_error_for_user tests are in util.rs
}
