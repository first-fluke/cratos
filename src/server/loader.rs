//! Configuration loading
//!
//! Handles loading configuration from embedded defaults, files, and environment.

use super::config::AppConfig;
use anyhow::{Context, Result};
use config::{Config, Environment, File, FileFormat};

/// Embedded default configuration (compiled into binary)
pub const DEFAULT_CONFIG: &str = include_str!("../../config/default.toml");

/// Load configuration from files and environment
pub fn load_config() -> Result<AppConfig> {
    let config = Config::builder()
        // 1. Embedded defaults (always available)
        .add_source(File::from_str(DEFAULT_CONFIG, FileFormat::Toml))
        // 2. External overrides (optional)
        .add_source(File::with_name("config/default").required(false))
        .add_source(
            File::with_name(&format!(
                "config/{}",
                std::env::var("CRATOS_ENV").unwrap_or_else(|_| "development".to_string())
            ))
            .required(false),
        )
        .add_source(File::with_name("config/local").required(false))
        // 3. Environment variables (highest priority)
        // prefix_separator("_") ensures CRATOS_LLM__X works (single _ after prefix).
        // Without it, config-rs 0.14 defaults prefix_separator to separator ("__"),
        // requiring CRATOS__LLM__X which doesn't match .env convention.
        .add_source(
            Environment::with_prefix("CRATOS")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true),
        )
        .build()
        .context("Failed to build configuration")?;

    config
        .try_deserialize()
        .context("Failed to deserialize configuration")
}
