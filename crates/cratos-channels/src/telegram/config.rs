//! Telegram configuration types

use crate::error::{Error, Result};

/// DM security policy for Telegram
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmPolicy {
    /// Require pairing code before accepting DMs from unknown users
    Pairing,
    /// Only accept DMs from users in the allowed_users list
    Allowlist,
    /// Accept DMs from any user (least secure)
    Open,
    /// Disable DM handling entirely
    Disabled,
}

impl Default for DmPolicy {
    fn default() -> Self {
        Self::Allowlist
    }
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
    /// DM security policy
    pub dm_policy: DmPolicy,
    /// Chat ID for system notifications (approval requests, errors, etc.)
    pub notify_chat_id: Option<i64>,
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

        let dm_policy = std::env::var("TELEGRAM_DM_POLICY")
            .ok()
            .map(|s| match s.to_lowercase().as_str() {
                "pairing" => DmPolicy::Pairing,
                "allowlist" => DmPolicy::Allowlist,
                "open" => DmPolicy::Open,
                "disabled" => DmPolicy::Disabled,
                _ => DmPolicy::default(),
            })
            .unwrap_or_default();

        let notify_chat_id = std::env::var("TELEGRAM_NOTIFY_CHAT_ID")
            .ok()
            .and_then(|s| s.trim().parse().ok());

        Ok(Self {
            bot_token,
            allowed_users,
            allowed_groups,
            groups_mention_only,
            dm_policy,
            notify_chat_id,
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
            dm_policy: DmPolicy::default(),
            notify_chat_id: None,
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

    /// Set DM security policy
    #[must_use]
    pub fn with_dm_policy(mut self, policy: DmPolicy) -> Self {
        self.dm_policy = policy;
        self
    }

    /// Set chat ID for system notifications
    #[must_use]
    pub fn with_notify_chat_id(mut self, chat_id: i64) -> Self {
        self.notify_chat_id = Some(chat_id);
        self
    }
}
