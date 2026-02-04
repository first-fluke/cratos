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
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ============================================================================
// Constants
// ============================================================================

/// Default batch size before sending events
const DEFAULT_BATCH_SIZE: usize = 10;

/// Default flush interval in seconds (5 minutes)
const DEFAULT_FLUSH_INTERVAL_SECS: u64 = 300;

/// Maximum queue size to prevent unbounded memory growth
const MAX_QUEUE_SIZE: usize = 1000;

/// HTTP request timeout in seconds
const HTTP_TIMEOUT_SECS: u64 = 10;

/// Environment variable name for telemetry enabled flag
const ENV_TELEMETRY_ENABLED: &str = "CRATOS_TELEMETRY_ENABLED";

/// Telemetry config file name
const CONFIG_FILE_NAME: &str = "telemetry.toml";

/// Cratos data directory name
const CRATOS_DIR_NAME: &str = ".cratos";

// ============================================================================
// Configuration
// ============================================================================

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
    std::env::var(ENV_TELEMETRY_ENABLED)
        .map(|v| !matches!(v.to_lowercase().as_str(), "false" | "0"))
        .unwrap_or(true)
}

fn generate_anonymous_id() -> String {
    Uuid::new_v4().to_string()
}

fn default_batch_size() -> usize {
    DEFAULT_BATCH_SIZE
}

fn default_flush_interval() -> u64 {
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

// ============================================================================
// Events
// ============================================================================

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
        /// Channel type (telegram, slack, matrix, etc.)
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

    /// Error occurred (category only, no details)
    ErrorOccurred {
        /// Error category
        category: String,
    },
}

/// Telemetry event with metadata (internal use)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TelemetryRecord {
    timestamp: DateTime<Utc>,
    anonymous_id: String,
    event: TelemetryEvent,
}

// ============================================================================
// Statistics
// ============================================================================

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
    /// Get command success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f64 {
        let total = self.commands_executed.load(Ordering::Relaxed);
        let succeeded = self.commands_succeeded.load(Ordering::Relaxed);

        if total == 0 {
            1.0
        } else {
            succeeded as f64 / total as f64
        }
    }

    /// Update stats based on event
    fn update(&self, event: &TelemetryEvent) {
        match event {
            TelemetryEvent::CommandExecuted { success, .. } => {
                self.commands_executed.fetch_add(1, Ordering::Relaxed);
                if *success {
                    self.commands_succeeded.fetch_add(1, Ordering::Relaxed);
                }
            }
            TelemetryEvent::LlmUsed { tokens, .. } => {
                self.tokens_used
                    .fetch_add(u64::from(*tokens), Ordering::Relaxed);
            }
            TelemetryEvent::ToolExecuted { .. } => {
                self.tools_executed.fetch_add(1, Ordering::Relaxed);
            }
            TelemetryEvent::SkillUsed { .. } => {
                self.skills_used.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

// ============================================================================
// Telemetry Manager
// ============================================================================

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
            info!("Telemetry enabled (opt-out via {}=false)", ENV_TELEMETRY_ENABLED);
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
            self.clear_queue().await;
        }
    }

    /// Clear the event queue
    async fn clear_queue(&self) {
        let mut queue = self.event_queue.write().await;
        queue.clear();
    }

    /// Track an event
    pub async fn track(&self, event: TelemetryEvent) {
        // Always update local stats, even if telemetry is disabled
        self.stats.update(&event);

        if !self.is_enabled() {
            return;
        }

        self.enqueue_event(event).await;
    }

    /// Enqueue event and flush if batch size reached
    async fn enqueue_event(&self, event: TelemetryEvent) {
        let config = self.config.read().await;

        let record = TelemetryRecord {
            timestamp: Utc::now(),
            anonymous_id: config.anonymous_id.clone(),
            event,
        };

        let should_flush = {
            let mut queue = self.event_queue.write().await;
            queue.push(record);
            queue.len() >= config.batch_size
        };

        drop(config);

        if should_flush {
            self.flush().await;
        }
    }

    // Convenience tracking methods

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

    /// Flush pending events to endpoint
    pub async fn flush(&self) {
        if !self.is_enabled() {
            return;
        }

        let events = self.drain_queue().await;
        if events.is_empty() {
            return;
        }

        let config = self.config.read().await;
        let endpoint = match &config.endpoint_url {
            Some(url) => url.clone(),
            None => {
                debug!("Telemetry: {} events (no endpoint configured)", events.len());
                return;
            }
        };
        drop(config);

        self.send_events_to_endpoint(&endpoint, events).await;
    }

    /// Drain all events from queue
    async fn drain_queue(&self) -> Vec<TelemetryRecord> {
        let mut queue = self.event_queue.write().await;
        queue.drain(..).collect()
    }

    /// Send events to endpoint, re-queue on failure
    async fn send_events_to_endpoint(&self, endpoint: &str, events: Vec<TelemetryRecord>) {
        match Self::send_http_request(endpoint, &events).await {
            Ok(()) => {
                debug!("Telemetry: Sent {} events", events.len());
            }
            Err(e) => {
                warn!("Telemetry: Failed to send events: {}", e);
                self.requeue_events(events).await;
            }
        }
    }

    /// Re-queue events on failure (with size limit)
    async fn requeue_events(&self, events: Vec<TelemetryRecord>) {
        let mut queue = self.event_queue.write().await;
        if queue.len() < MAX_QUEUE_SIZE {
            queue.extend(events);
        }
    }

    /// Send HTTP request to endpoint
    async fn send_http_request(endpoint: &str, events: &[TelemetryRecord]) -> Result<(), String> {
        let client = reqwest::Client::new();

        let response = client
            .post(endpoint)
            .json(events)
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP {}", response.status()))
        }
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Global Instance
// ============================================================================

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config(enabled: bool, batch_size: usize) -> TelemetryConfig {
        TelemetryConfig {
            enabled,
            batch_size,
            ..Default::default()
        }
    }

    #[test]
    fn test_default_config() {
        let config = TelemetryConfig::default();

        assert!(!config.anonymous_id.is_empty());
        assert_eq!(config.batch_size, DEFAULT_BATCH_SIZE);
        assert_eq!(config.flush_interval_secs, DEFAULT_FLUSH_INTERVAL_SECS);
    }

    #[tokio::test]
    async fn test_telemetry_disabled_does_not_queue() {
        let telemetry = Telemetry::new(create_test_config(false, DEFAULT_BATCH_SIZE));
        assert!(!telemetry.is_enabled());

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
    async fn test_telemetry_enabled_queues_events() {
        let telemetry = Telemetry::new(create_test_config(true, 100));
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
    async fn test_stats_update_even_when_disabled() {
        let telemetry = Telemetry::new(create_test_config(false, DEFAULT_BATCH_SIZE));

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
        assert_eq!(telemetry.stats.commands_succeeded.load(Ordering::Relaxed), 1);
        assert!((telemetry.stats.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_set_enabled_clears_queue_when_disabled() {
        let telemetry = Telemetry::new(create_test_config(true, 100));

        telemetry
            .track(TelemetryEvent::FeatureUsed {
                feature: "test".to_string(),
            })
            .await;

        {
            let queue = telemetry.event_queue.read().await;
            assert_eq!(queue.len(), 1);
        }

        telemetry.set_enabled(false).await;

        {
            let queue = telemetry.event_queue.read().await;
            assert!(queue.is_empty());
        }
    }

    #[tokio::test]
    async fn test_toggle_enabled() {
        let telemetry = Telemetry::new(create_test_config(true, DEFAULT_BATCH_SIZE));

        assert!(telemetry.is_enabled());

        telemetry.set_enabled(false).await;
        assert!(!telemetry.is_enabled());

        telemetry.set_enabled(true).await;
        assert!(telemetry.is_enabled());
    }
}
