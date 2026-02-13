//! Production configuration validation
//!
//! Security checks for production deployments.

use super::config::AppConfig;
use anyhow::Result;
use tracing::warn;

/// Validate configuration for production security
pub fn validate_production_config(config: &AppConfig) -> Result<()> {
    let is_production = std::env::var("CRATOS_ENV")
        .map(|v| v.to_lowercase() == "production")
        .unwrap_or(false);

    if !is_production {
        return Ok(());
    }

    if config.server.host == "0.0.0.0" {
        warn!(
            "SECURITY WARNING: Server is binding to all interfaces (0.0.0.0) in production. \
             Consider binding to 127.0.0.1 and using a reverse proxy."
        );
    }

    if !config.server.auth.enabled && config.server.host != "127.0.0.1" {
        warn!(
            "SECURITY WARNING: Authentication is DISABLED while server is exposed externally. \
             Enable [server.auth] enabled = true in production!"
        );
    }

    if config.redis.url.starts_with("redis://") && !config.redis.url.contains('@') && is_production
    {
        warn!(
            "SECURITY WARNING: Redis connection appears to have no authentication in production. \
             Consider enabling Redis AUTH."
        );
    }

    let required_env_vars = ["TELEGRAM_BOT_TOKEN"];
    for var in required_env_vars {
        if config.channels.telegram.enabled && std::env::var(var).is_err() {
            warn!(
                "SECURITY WARNING: {} is not set but Telegram is enabled. \
                 Bot may not function correctly.",
                var
            );
        }
    }

    Ok(())
}
