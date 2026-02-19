//! Slack - slack-morphism adapter
//!
//! This module provides the Slack bot adapter using the slack-morphism library
//! with Socket Mode support for real-time event handling.

use crate::error::{Error, Result};
use cratos_core::Orchestrator;
use slack_morphism::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Slack API client and message sending logic.
pub mod api;
/// Socket Mode event handlers.
pub mod events;
/// Slack-specific message formatting (blocks).
pub mod formatting;

#[cfg(test)]
mod tests;

/// Maximum allowed timestamp age in seconds (5 minutes)
pub(crate) const MAX_TIMESTAMP_AGE_SECS: u64 = 300;

/// Constant-time comparison to prevent timing attacks
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Slack bot configuration
#[derive(Debug, Clone)]
pub struct SlackConfig {
    /// Bot token (xoxb-...)
    pub bot_token: String,
    /// App token for Socket Mode (xapp-...)
    pub app_token: String,
    /// Signing secret for request verification
    pub signing_secret: String,
    /// Allowed workspace IDs (empty = allow all)
    pub allowed_workspaces: Vec<String>,
    /// Allowed channel IDs (empty = allow all)
    pub allowed_channels: Vec<String>,
    /// Whether to respond only to mentions/direct messages
    pub mentions_only: bool,
}

impl SlackConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let bot_token = std::env::var("SLACK_BOT_TOKEN")
            .map_err(|_| Error::Slack("SLACK_BOT_TOKEN not set".to_string()))?;

        let app_token = std::env::var("SLACK_APP_TOKEN")
            .map_err(|_| Error::Slack("SLACK_APP_TOKEN not set".to_string()))?;

        let signing_secret = std::env::var("SLACK_SIGNING_SECRET")
            .map_err(|_| Error::Slack("SLACK_SIGNING_SECRET not set".to_string()))?;

        let allowed_workspaces: Vec<String> = std::env::var("SLACK_ALLOWED_WORKSPACES")
            .ok()
            .map(|s| s.split(',').map(|id| id.trim().to_string()).collect())
            .unwrap_or_default();

        let allowed_channels: Vec<String> = std::env::var("SLACK_ALLOWED_CHANNELS")
            .ok()
            .map(|s| s.split(',').map(|id| id.trim().to_string()).collect())
            .unwrap_or_default();

        let mentions_only = std::env::var("SLACK_MENTIONS_ONLY")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(true);

        Ok(Self {
            bot_token,
            app_token,
            signing_secret,
            allowed_workspaces,
            allowed_channels,
            mentions_only,
        })
    }

    /// Create with a bot token
    #[must_use]
    pub fn new(
        bot_token: impl Into<String>,
        app_token: impl Into<String>,
        signing_secret: impl Into<String>,
    ) -> Self {
        Self {
            bot_token: bot_token.into(),
            app_token: app_token.into(),
            signing_secret: signing_secret.into(),
            allowed_workspaces: Vec::new(),
            allowed_channels: Vec::new(),
            mentions_only: true,
        }
    }

    /// Set allowed workspaces
    #[must_use]
    pub fn with_allowed_workspaces(mut self, workspaces: Vec<String>) -> Self {
        self.allowed_workspaces = workspaces;
        self
    }

    /// Set allowed channels
    #[must_use]
    pub fn with_allowed_channels(mut self, channels: Vec<String>) -> Self {
        self.allowed_channels = channels;
        self
    }

    /// Set mentions only mode
    #[must_use]
    pub fn with_mentions_only(mut self, enabled: bool) -> Self {
        self.mentions_only = enabled;
        self
    }
}

/// Slack bot adapter with Socket Mode support
pub struct SlackAdapter {
    pub(crate) config: SlackConfig,
    pub(crate) bot_user_id: RwLock<Option<String>>,
}

impl SlackAdapter {
    /// Create a new Slack adapter
    #[must_use]
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            bot_user_id: RwLock::new(None),
        }
    }

    /// Create from environment
    pub fn from_env() -> Result<Self> {
        let config = SlackConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Get the bot token
    pub(crate) fn bot_token(&self) -> SlackApiToken {
        SlackApiToken::new(self.config.bot_token.clone().into())
    }

    /// Get the app token (for Socket Mode)
    pub(crate) fn app_token(&self) -> SlackApiToken {
        SlackApiToken::new(self.config.app_token.clone().into())
    }

    /// Get the cached bot user ID
    pub async fn get_bot_user_id(&self) -> Option<String> {
        self.bot_user_id.read().await.clone()
    }

    /// Set the bot user ID
    pub(crate) async fn set_bot_user_id(&self, user_id: String) {
        let mut guard = self.bot_user_id.write().await;
        *guard = Some(user_id);
    }

    /// Check if a workspace is allowed
    pub fn is_workspace_allowed(&self, workspace_id: &str) -> bool {
        self.config.allowed_workspaces.is_empty()
            || self
                .config
                .allowed_workspaces
                .contains(&workspace_id.to_string())
    }

    /// Check if a channel is allowed
    pub fn is_channel_allowed(&self, channel_id: &str) -> bool {
        self.config.allowed_channels.is_empty()
            || self
                .config
                .allowed_channels
                .contains(&channel_id.to_string())
    }

    /// Check if the message mentions the bot
    pub async fn is_bot_mentioned(&self, text: &str) -> bool {
        if let Some(bot_id) = self.get_bot_user_id().await {
            text.contains(&format!("<@{}>", bot_id))
        } else {
            false
        }
    }

    /// Start the bot in Socket Mode with the given orchestrator.
    ///
    /// Connects to Slack via WebSocket (Socket Mode), listens for
    /// message and app_mention events, and routes them to the orchestrator.
    pub async fn run(self: Arc<Self>, orchestrator: Arc<Orchestrator>) -> Result<()> {
        info!("Starting Slack adapter in Socket Mode");

        // Fetch bot info first
        self.fetch_bot_info().await?;

        info!(
            bot_user_id = ?self.get_bot_user_id().await,
            "Slack adapter ready, starting Socket Mode listener"
        );

        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::Slack(format!("HTTP connector: {}", e)))?;
        let client = Arc::new(SlackClient::new(connector));

        let callbacks = SlackSocketModeListenerCallbacks::new()
            .with_push_events(events::socket_mode_push_handler)
            .with_interaction_events(events::socket_mode_interaction_handler);

        let user_state = events::SocketModeState {
            adapter: self.clone(),
            orchestrator: orchestrator.clone(),
        };

        let listener_env = Arc::new(
            SlackClientEventsListenerEnvironment::new(client.clone()).with_user_state(user_state),
        );

        let listener = SlackClientSocketModeListener::new(
            &SlackClientSocketModeConfig::new(),
            listener_env,
            callbacks,
        );

        let app_token = self.app_token();
        listener
            .listen_for(&app_token)
            .await
            .map_err(|e| Error::Slack(format!("Socket Mode listen: {}", e)))?;

        info!("Socket Mode connected, serving events...");
        listener.serve().await;

        info!("Slack adapter shutdown complete");
        Ok(())
    }
}
