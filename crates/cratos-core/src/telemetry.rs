//! Telemetry - Usage Statistics Collection
//!
//! This module provides opt-out telemetry for Cratos usage statistics.
//! Telemetry is **enabled by default** but can be disabled in settings.
//!
//! ## What We Collect
//!
//! - Command usage counts (anonymized)
//! - Feature usage patterns
//! - Error rates (no error details)
//! - Performance metrics (response times)
//!
//! ## What We DON'T Collect
//!
//! - Message contents
//! - User identifiers
//! - API keys or credentials
//! - File contents
//! - Personal information
//!
//! ## Configuration
//!
//! ```toml
//! [telemetry]
//! enabled = true  # Set to false to opt-out
//! anonymous_id = "auto"  # Auto-generated anonymous ID
//! ```
//!
//! Or via environment variable:
//! ```bash
//! export CRATOS_TELEMETRY_ENABLED=false
//! ```
//!
//! ## Example
//!
//! ```ignore
//! use cratos_core::telemetry::{Telemetry, TelemetryConfig, TelemetryEvent};
//!
//! // Initialize telemetry (respects config)
//! let telemetry = Telemetry::new(TelemetryConfig::default());
//!
//! // Track an event
//! telemetry.track(TelemetryEvent::CommandExecuted {
//!     command: "summarize".to_string(),
//!     duration_ms: 1500,
//!     success: true,
//! }).await;
//!
//! // Check if telemetry is enabled
//! if telemetry.is_enabled() {
//!     println!("Telemetry is enabled");
//! }
//!
//! // Disable telemetry
//! telemetry.set_enabled(false);
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

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

fn default_enabled() -> bool {
    // Check environment variable first
    std::env::var("CRATOS_TELEMETRY_ENABLED")
        .map(|v| v.to_lowercase() != "false" && v != "0")
        .unwrap_or(true) // Default: enabled
}

fn generate_anonymous_id() -> String {
    // Try to load existing ID from config, otherwise generate new
    Uuid::new_v4().to_string()
}

fn default_batch_size() -> usize {
    10
}

fn default_flush_interval() -> u64 {
    300 // 5 minutes
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
            match std::fs::read_to_string(&config_path) {
                Ok(content) => {
                    if let Ok(config) = toml::from_str::<TelemetryConfig>(&content) {
                        return config;
                    }
                }
                Err(e) => {
                    warn!("Failed to read telemetry config: {}", e);
                }
            }
        }

        // Generate default config and save
        let config = Self::default();
        let _ = config.save();
        config
    }

    /// Save config to file
    pub fn save(&self) -> std::io::Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        std::fs::write(config_path, content)
    }

    /// Get config file path
    fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cratos")
            .join("telemetry.toml")
    }
}

/// Telemetry event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelemetryEvent {
    /// Application started
    AppStarted {
        /// Cratos version
        version: String,
        /// Platform (darwin, linux, windows)
        platform: String,
    },

    /// Command executed
    CommandExecuted {
        /// Command name (anonymized)
        command: String,
        /// Execution duration in milliseconds
        duration_ms: u64,
        /// Whether the command succeeded
        success: bool,
    },

    /// Channel used
    ChannelUsed {
        /// Channel type (telegram, slack, etc.)
        channel_type: String,
    },

    /// LLM provider used
    LlmUsed {
        /// Provider name (openai, anthropic, etc.)
        provider: String,
        /// Model tier (fast, standard, premium)
        tier: String,
        /// Token count
        tokens: u32,
    },

    /// Tool executed
    ToolExecuted {
        /// Tool category (file, http, git, etc.)
        category: String,
        /// Execution duration in milliseconds
        duration_ms: u64,
        /// Whether the tool succeeded
        success: bool,
    },

    /// Skill used
    SkillUsed {
        /// Skill origin (builtin, user_defined, auto_generated)
        origin: String,
        /// Whether the skill succeeded
        success: bool,
    },

    /// Feature used
    FeatureUsed {
        /// Feature name
        feature: String,
    },

    /// Error occurred (no details, just category)
    ErrorOccurred {
        /// Error category
        category: String,
    },
}

/// Telemetry event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TelemetryRecord {
    /// Event timestamp
    timestamp: DateTime<Utc>,
    /// Anonymous ID
    anonymous_id: String,
    /// Event data
    event: TelemetryEvent,
}

/// Aggregated statistics (local only, never sent)
#[derive(Debug, Default)]
pub struct TelemetryStats {
    /// Total commands executed
    pub commands_executed: AtomicU64,
    /// Total successful commands
    pub commands_succeeded: AtomicU64,
    /// Total LLM tokens used
    pub tokens_used: AtomicU64,
    /// Total tools executed
    pub tools_executed: AtomicU64,
    /// Total skills used
    pub skills_used: AtomicU64,
}

impl TelemetryStats {
    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.commands_executed.load(Ordering::Relaxed);
        let succeeded = self.commands_succeeded.load(Ordering::Relaxed);

        if total == 0 {
            1.0
        } else {
            succeeded as f64 / total as f64
        }
    }
}

/// Telemetry manager
pub struct Telemetry {
    config: RwLock<TelemetryConfig>,
    enabled: AtomicBool,
    event_queue: RwLock<Vec<TelemetryRecord>>,
    stats: TelemetryStats,
}

impl Telemetry {
    /// Create a new telemetry instance
    pub fn new(config: TelemetryConfig) -> Self {
        let enabled = config.enabled;

        if enabled {
            info!("Telemetry enabled (opt-out via CRATOS_TELEMETRY_ENABLED=false)");
        } else {
            info!("Telemetry disabled");
        }

        Self {
            config: RwLock::new(config),
            enabled: AtomicBool::new(enabled),
            event_queue: RwLock::new(Vec::new()),
            stats: TelemetryStats::default(),
        }
    }

    /// Create with default config (loads from file or env)
    pub fn with_defaults() -> Self {
        Self::new(TelemetryConfig::load())
    }

    /// Check if telemetry is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Enable or disable telemetry
    pub async fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);

        let mut config = self.config.write().await;
        config.enabled = enabled;
        let _ = config.save();

        if enabled {
            info!("Telemetry enabled");
        } else {
            info!("Telemetry disabled");

            // Clear pending events
            let mut queue = self.event_queue.write().await;
            queue.clear();
        }
    }

    /// Track an event
    pub async fn track(&self, event: TelemetryEvent) {
        // Update local stats (always, even if telemetry disabled)
        self.update_stats(&event);

        // Only queue if enabled
        if !self.is_enabled() {
            return;
        }

        let config = self.config.read().await;

        let record = TelemetryRecord {
            timestamp: Utc::now(),
            anonymous_id: config.anonymous_id.clone(),
            event,
        };

        let mut queue = self.event_queue.write().await;
        queue.push(record);

        // Flush if batch size reached
        if queue.len() >= config.batch_size {
            drop(queue);
            drop(config);
            self.flush().await;
        }
    }

    /// Track app start
    pub async fn track_app_start(&self) {
        self.track(TelemetryEvent::AppStarted {
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
        })
        .await;
    }

    /// Track command execution
    pub async fn track_command(&self, command: &str, duration_ms: u64, success: bool) {
        self.track(TelemetryEvent::CommandExecuted {
            command: command.to_string(),
            duration_ms,
            success,
        })
        .await;
    }

    /// Track channel usage
    pub async fn track_channel(&self, channel_type: &str) {
        self.track(TelemetryEvent::ChannelUsed {
            channel_type: channel_type.to_string(),
        })
        .await;
    }

    /// Track LLM usage
    pub async fn track_llm(&self, provider: &str, tier: &str, tokens: u32) {
        self.track(TelemetryEvent::LlmUsed {
            provider: provider.to_string(),
            tier: tier.to_string(),
            tokens,
        })
        .await;
    }

    /// Track tool execution
    pub async fn track_tool(&self, category: &str, duration_ms: u64, success: bool) {
        self.track(TelemetryEvent::ToolExecuted {
            category: category.to_string(),
            duration_ms,
            success,
        })
        .await;
    }

    /// Track skill usage
    pub async fn track_skill(&self, origin: &str, success: bool) {
        self.track(TelemetryEvent::SkillUsed {
            origin: origin.to_string(),
            success,
        })
        .await;
    }

    /// Track feature usage
    pub async fn track_feature(&self, feature: &str) {
        self.track(TelemetryEvent::FeatureUsed {
            feature: feature.to_string(),
        })
        .await;
    }

    /// Track error (category only, no details)
    pub async fn track_error(&self, category: &str) {
        self.track(TelemetryEvent::ErrorOccurred {
            category: category.to_string(),
        })
        .await;
    }

    /// Get local statistics
    pub fn stats(&self) -> &TelemetryStats {
        &self.stats
    }

    /// Flush pending events (send to endpoint)
    pub async fn flush(&self) {
        if !self.is_enabled() {
            return;
        }

        let mut queue = self.event_queue.write().await;

        if queue.is_empty() {
            return;
        }

        let events: Vec<TelemetryRecord> = queue.drain(..).collect();
        drop(queue);

        let config = self.config.read().await;

        // If no endpoint configured, just log locally
        if config.endpoint_url.is_none() {
            debug!("Telemetry: {} events (no endpoint configured)", events.len());
            return;
        }

        // Send to endpoint
        let endpoint = config.endpoint_url.as_ref().unwrap();

        match Self::send_events(endpoint, &events).await {
            Ok(_) => {
                debug!("Telemetry: Sent {} events", events.len());
            }
            Err(e) => {
                warn!("Telemetry: Failed to send events: {}", e);
                // Re-queue events on failure (with limit)
                let mut queue = self.event_queue.write().await;
                if queue.len() < 1000 {
                    // Prevent unbounded growth
                    queue.extend(events);
                }
            }
        }
    }

    /// Send events to endpoint
    async fn send_events(endpoint: &str, events: &[TelemetryRecord]) -> Result<(), String> {
        let client = reqwest::Client::new();

        let response = client
            .post(endpoint)
            .json(events)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP {}", response.status()))
        }
    }

    /// Update local statistics
    fn update_stats(&self, event: &TelemetryEvent) {
        match event {
            TelemetryEvent::CommandExecuted { success, .. } => {
                self.stats.commands_executed.fetch_add(1, Ordering::Relaxed);
                if *success {
                    self.stats.commands_succeeded.fetch_add(1, Ordering::Relaxed);
                }
            }
            TelemetryEvent::LlmUsed { tokens, .. } => {
                self.stats
                    .tokens_used
                    .fetch_add(*tokens as u64, Ordering::Relaxed);
            }
            TelemetryEvent::ToolExecuted { .. } => {
                self.stats.tools_executed.fetch_add(1, Ordering::Relaxed);
            }
            TelemetryEvent::SkillUsed { .. } => {
                self.stats.skills_used.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Global telemetry instance
static TELEMETRY: std::sync::OnceLock<Telemetry> = std::sync::OnceLock::new();

/// Get global telemetry instance
pub fn global_telemetry() -> &'static Telemetry {
    TELEMETRY.get_or_init(Telemetry::with_defaults)
}

/// Initialize global telemetry with custom config
pub fn init_telemetry(config: TelemetryConfig) {
    let _ = TELEMETRY.set(Telemetry::new(config));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TelemetryConfig::default();
        // Default depends on env var, but should have valid anonymous_id
        assert!(!config.anonymous_id.is_empty());
        assert_eq!(config.batch_size, 10);
        assert_eq!(config.flush_interval_secs, 300);
    }

    #[tokio::test]
    async fn test_telemetry_disabled() {
        let config = TelemetryConfig {
            enabled: false,
            ..Default::default()
        };

        let telemetry = Telemetry::new(config);
        assert!(!telemetry.is_enabled());

        // Events should not be queued when disabled
        telemetry
            .track(TelemetryEvent::CommandExecuted {
                command: "test".to_string(),
                duration_ms: 100,
                success: true,
            })
            .await;

        let queue = telemetry.event_queue.read().await;
        assert!(queue.is_empty());
    }

    #[tokio::test]
    async fn test_telemetry_enabled() {
        let config = TelemetryConfig {
            enabled: true,
            batch_size: 100, // High batch size to prevent auto-flush
            ..Default::default()
        };

        let telemetry = Telemetry::new(config);
        assert!(telemetry.is_enabled());

        telemetry
            .track(TelemetryEvent::CommandExecuted {
                command: "test".to_string(),
                duration_ms: 100,
                success: true,
            })
            .await;

        let queue = telemetry.event_queue.read().await;
        assert_eq!(queue.len(), 1);
    }

    #[tokio::test]
    async fn test_stats_update() {
        let config = TelemetryConfig {
            enabled: false, // Disabled, but stats should still update
            ..Default::default()
        };

        let telemetry = Telemetry::new(config);

        telemetry
            .track(TelemetryEvent::CommandExecuted {
                command: "test".to_string(),
                duration_ms: 100,
                success: true,
            })
            .await;

        telemetry
            .track(TelemetryEvent::CommandExecuted {
                command: "test".to_string(),
                duration_ms: 100,
                success: false,
            })
            .await;

        assert_eq!(telemetry.stats.commands_executed.load(Ordering::Relaxed), 2);
        assert_eq!(
            telemetry.stats.commands_succeeded.load(Ordering::Relaxed),
            1
        );
        assert!((telemetry.stats.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_set_enabled() {
        let telemetry = Telemetry::new(TelemetryConfig {
            enabled: true,
            ..Default::default()
        });

        assert!(telemetry.is_enabled());

        telemetry.set_enabled(false).await;
        assert!(!telemetry.is_enabled());

        telemetry.set_enabled(true).await;
        assert!(telemetry.is_enabled());
    }
}
