use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::warn;
use uuid::Uuid;

// ============================================================================
// Constants
// ============================================================================

/// Default batch size before sending events
pub const DEFAULT_BATCH_SIZE: usize = 10;

/// Default flush interval in seconds (5 minutes)
pub const DEFAULT_FLUSH_INTERVAL_SECS: u64 = 300;

/// Environment variable name for telemetry enabled flag
pub const ENV_TELEMETRY_ENABLED: &str = "CRATOS_TELEMETRY_ENABLED";

/// Telemetry config file name
pub const CONFIG_FILE_NAME: &str = "telemetry.toml";

/// Cratos data directory name
pub const CRATOS_DIR_NAME: &str = ".cratos";

/// Telemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Whether telemetry is enabled (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Anonymous identifier (auto-generated if not set)
    #[serde(default = "generate_anonymous_id")]
    pub anonymous_id: String,

    /// Telemetry endpoint URL (optional, for self-hosted)
    #[serde(default)]
    pub endpoint_url: Option<String>,

    /// Batch size before sending (default: 10)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Flush interval in seconds (default: 300 = 5 minutes)
    #[serde(default = "default_flush_interval")]
    pub flush_interval_secs: u64,
}

/// Returns whether telemetry is enabled (checks CRATOS_TELEMETRY_ENABLED env var).
pub fn default_enabled() -> bool {
    std::env::var(ENV_TELEMETRY_ENABLED)
        .map(|v| !matches!(v.to_lowercase().as_str(), "false" | "0"))
        .unwrap_or(true)
}

/// Generates a random anonymous UUID for telemetry identification.
pub fn generate_anonymous_id() -> String {
    Uuid::new_v4().to_string()
}

/// Returns the default batch size for telemetry event batching.
pub fn default_batch_size() -> usize {
    DEFAULT_BATCH_SIZE
}

/// Returns the default flush interval in seconds for telemetry batches.
pub fn default_flush_interval() -> u64 {
    DEFAULT_FLUSH_INTERVAL_SECS
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            anonymous_id: generate_anonymous_id(),
            endpoint_url: None,
            batch_size: default_batch_size(),
            flush_interval_secs: default_flush_interval(),
        }
    }
}

impl TelemetryConfig {
    /// Load from config file or use defaults
    pub fn load() -> Self {
        let config_path = Self::config_path();

        if config_path.exists() {
            if let Some(config) = Self::load_from_file(&config_path) {
                return config;
            }
        }

        let config = Self::default();
        let _ = config.save();
        config
    }

    /// Load config from file path
    fn load_from_file(path: &PathBuf) -> Option<Self> {
        match std::fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).ok(),
            Err(e) => {
                warn!("Failed to read telemetry config: {}", e);
                None
            }
        }
    }

    /// Save config to file
    pub fn save(&self) -> std::io::Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self).map_err(std::io::Error::other)?;

        std::fs::write(config_path, content)
    }

    /// Get config file path
    fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CRATOS_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    }
}
