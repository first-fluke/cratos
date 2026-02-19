use crate::error::{Error, Result};
use serde::Deserialize;

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
