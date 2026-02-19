use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::config::{TelemetryConfig, ENV_TELEMETRY_ENABLED};
use super::types::{TelemetryEvent, TelemetryRecord};
use super::stats::TelemetryStats;

/// Maximum queue size to prevent unbounded memory growth
const MAX_QUEUE_SIZE: usize = 1000;

/// HTTP request timeout in seconds
const HTTP_TIMEOUT_SECS: u64 = 10;

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
            info!(
                "Telemetry enabled (opt-out via {}=false)",
                ENV_TELEMETRY_ENABLED
            );
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
                debug!(
                    "Telemetry: {} events (no endpoint configured)",
                    events.len()
                );
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
