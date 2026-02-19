use super::config::DiscordConfig;
use super::handler::DiscordHandler;
use crate::error::{Error, Result};
use crate::message::{
    ChannelAdapter, ChannelType, NormalizedMessage, OutgoingAttachment, OutgoingMessage,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cratos_core::event_bus::OrchestratorEvent;
use cratos_core::Orchestrator;
use serenity::all::{
    ButtonStyle, ChannelId, Client, CreateActionRow, CreateAttachment, CreateButton, CreateEmbed,
    CreateMessage, EditMessage, GatewayIntents, Message, MessageId, MessageReference,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// Discord bot adapter
pub struct DiscordAdapter {
    pub(crate) config: DiscordConfig,
    pub(crate) bot_user_id: AtomicU64,
    pub(crate) http: RwLock<Option<Arc<serenity::http::Http>>>,
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
                                .description(format!(
                                    "Execution `{}` requires approval.",
                                    execution_id
                                ))
                                .field("Request ID", request_id.to_string(), true)
                                .color(0xffaa00);
                            let approve_btn = CreateButton::new(format!("approve:{}", request_id))
                                .label("Approve")
                                .style(ButtonStyle::Success);
                            let deny_btn = CreateButton::new(format!("deny:{}", request_id))
                                .label("Deny")
                                .style(ButtonStyle::Danger);
                            let row = CreateActionRow::Buttons(vec![approve_btn, deny_btn]);
                            let msg = CreateMessage::new().embed(embed).components(vec![row]);
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
            info!(
                "Discord EventBus notification listener started (channel: {})",
                notify_ch
            );
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

    async fn send_attachment(
        &self,
        channel_id: &str,
        attachment: OutgoingAttachment,
        reply_to: Option<&str>,
    ) -> Result<String> {
        let channel_id: u64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid channel ID".to_string()))?;

        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| Error::Discord("Not connected".to_string()))?;

        // Decode base64 data
        let data = BASE64
            .decode(&attachment.data)
            .map_err(|e| Error::Discord(format!("Invalid base64 data: {}", e)))?;

        let channel = ChannelId::new(channel_id);

        // Create attachment
        let discord_attachment = CreateAttachment::bytes(data, &attachment.filename);

        // Build message with attachment
        let content = attachment.caption.as_deref().unwrap_or("").to_string();

        let mut builder = CreateMessage::new().add_file(discord_attachment);

        if !content.is_empty() {
            builder = builder.content(&content);
        }

        if let Some(reply) = reply_to {
            if let Ok(msg_id) = reply.parse::<u64>() {
                builder = builder.reference_message(MessageReference::from((
                    ChannelId::new(channel_id),
                    MessageId::new(msg_id),
                )));
            }
        }

        let sent = channel
            .send_message(http, builder)
            .await
            .map_err(|e| Error::Discord(format!("Failed to send attachment: {}", e)))?;

        Ok(sent.id.get().to_string())
    }
}
