//! Discord - serenity adapter

pub mod adapter;
pub mod commands;
pub mod config;
pub mod handler;

pub use adapter::DiscordAdapter;
pub use config::DiscordConfig;

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
}
