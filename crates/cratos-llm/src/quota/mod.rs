//! Quota & Rate Limit Tracking
//!
//! Tracks per-provider rate limit state from HTTP response headers.
//! Supports Anthropic (`anthropic-ratelimit-*`) and OpenAI-compatible
//! (`x-ratelimit-*`) header formats.

use chrono::{DateTime, Utc};
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(crate) mod providers;

pub(crate) use providers::anthropic::parse_anthropic_headers;
pub(crate) use providers::openai::{parse_generic_headers, parse_openai_headers};

// ============================================================================
// Types
// ============================================================================

/// How the quota information was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaSource {
    /// Parsed from HTTP response headers (most providers).
    ResponseHeaders,
    /// Polled from a dedicated quota API (e.g. Gemini).
    PolledApi,
    /// Derived from a 429 retry-after header.
    RetryAfter,
}

/// Rate limit state for a single provider.
#[derive(Debug, Clone)]
pub struct QuotaState {
    /// Provider name (e.g. "anthropic", "groq")
    pub provider: String,
    /// Remaining requests in current window
    pub requests_remaining: Option<u64>,
    /// Total request limit for current window
    pub requests_limit: Option<u64>,
    /// Remaining tokens in current window
    pub tokens_remaining: Option<u64>,
    /// Total token limit for current window
    pub tokens_limit: Option<u64>,
    /// When the current rate limit window resets
    pub reset_at: Option<DateTime<Utc>>,
    /// When this state was last updated
    pub updated_at: DateTime<Utc>,
    /// How this state was obtained.
    pub source: QuotaSource,
    /// Remaining fraction (0.0–1.0), if known from a polled API.
    pub remaining_fraction: Option<f64>,
    /// Tier label (e.g. "free", "pay-as-you-go"), if known.
    pub tier_label: Option<String>,
}

impl QuotaState {
    /// Percentage of requests used (0.0 – 100.0), or `None` if unknown.
    #[must_use]
    pub fn requests_usage_pct(&self) -> Option<f64> {
        match (self.requests_remaining, self.requests_limit) {
            (Some(rem), Some(limit)) if limit > 0 => {
                Some((1.0 - rem as f64 / limit as f64) * 100.0)
            }
            _ => None,
        }
    }

    /// Percentage of tokens used (0.0 – 100.0), or `None` if unknown.
    #[must_use]
    pub fn tokens_usage_pct(&self) -> Option<f64> {
        match (self.tokens_remaining, self.tokens_limit) {
            (Some(rem), Some(limit)) if limit > 0 => {
                Some((1.0 - rem as f64 / limit as f64) * 100.0)
            }
            _ => None,
        }
    }

    /// Remaining percentage (0.0–100.0), preferring `remaining_fraction`
    /// if available, otherwise computed from requests remaining/limit.
    #[must_use]
    pub fn remaining_pct(&self) -> Option<f64> {
        self.remaining_fraction.map(|f| f * 100.0).or_else(|| {
            match (self.requests_remaining, self.requests_limit) {
                (Some(r), Some(l)) if l > 0 => Some(r as f64 / l as f64 * 100.0),
                _ => None,
            }
        })
    }

    /// Returns `true` when remaining requests or tokens drop below
    /// `threshold` percent of the limit (default check: 20%).
    #[must_use]
    pub fn is_near_limit(&self, threshold_pct: f64) -> bool {
        if let Some(pct) = self.requests_usage_pct() {
            if pct >= (100.0 - threshold_pct) {
                return true;
            }
        }
        if let Some(pct) = self.tokens_usage_pct() {
            if pct >= (100.0 - threshold_pct) {
                return true;
            }
        }
        false
    }
}

/// Intermediate struct populated by a header parser before merging.
#[derive(Debug, Default)]
pub(crate) struct PartialQuotaState {
    pub(crate) requests_remaining: Option<u64>,
    pub(crate) requests_limit: Option<u64>,
    pub(crate) tokens_remaining: Option<u64>,
    pub(crate) tokens_limit: Option<u64>,
    pub(crate) reset_at: Option<DateTime<Utc>>,
}

impl PartialQuotaState {
    /// Returns `true` if at least one field was parsed.
    pub(crate) fn has_data(&self) -> bool {
        self.requests_remaining.is_some()
            || self.requests_limit.is_some()
            || self.tokens_remaining.is_some()
            || self.tokens_limit.is_some()
            || self.reset_at.is_some()
    }
}

// ============================================================================
// QuotaTracker
// ============================================================================

/// Thread-safe, in-memory tracker for provider rate limit states.
#[derive(Debug)]
pub struct QuotaTracker {
    states: RwLock<HashMap<String, QuotaState>>,
}

impl QuotaTracker {
    /// Create a new empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
        }
    }

    /// Parse provider-specific headers and merge into the tracked state.
    ///
    /// This should be called immediately after receiving an HTTP response,
    /// **before** consuming the body.
    pub async fn update_from_headers(&self, provider: &str, headers: &HeaderMap) {
        let partial = match provider {
            "anthropic" => parse_anthropic_headers(headers),
            "openai" | "deepseek" | "groq" | "fireworks" | "openrouter" | "siliconflow"
            | "novita" => parse_openai_headers(headers),
            _ => parse_generic_headers(headers),
        };

        if !partial.has_data() {
            return;
        }

        let now = Utc::now();
        let mut states = self.states.write().await;
        let state = states
            .entry(provider.to_string())
            .or_insert_with(|| QuotaState {
                provider: provider.to_string(),
                requests_remaining: None,
                requests_limit: None,
                tokens_remaining: None,
                tokens_limit: None,
                reset_at: None,
                updated_at: now,
                source: QuotaSource::ResponseHeaders,
                remaining_fraction: None,
                tier_label: None,
            });

        if let Some(v) = partial.requests_remaining {
            state.requests_remaining = Some(v);
        }
        if let Some(v) = partial.requests_limit {
            state.requests_limit = Some(v);
        }
        if let Some(v) = partial.tokens_remaining {
            state.tokens_remaining = Some(v);
        }
        if let Some(v) = partial.tokens_limit {
            state.tokens_limit = Some(v);
        }
        if let Some(v) = partial.reset_at {
            state.reset_at = Some(v);
        }
        state.updated_at = now;
    }

    /// Update state when a 429 (rate limited) response with retry-after is received.
    pub async fn update_from_retry_after(&self, provider: &str, retry_secs: u64) {
        let now = Utc::now();
        let reset_at = now + chrono::Duration::seconds(retry_secs as i64);
        let mut states = self.states.write().await;
        let state = states
            .entry(provider.to_string())
            .or_insert_with(|| QuotaState {
                provider: provider.to_string(),
                requests_remaining: None,
                requests_limit: None,
                tokens_remaining: None,
                tokens_limit: None,
                reset_at: None,
                updated_at: now,
                source: QuotaSource::RetryAfter,
                remaining_fraction: None,
                tier_label: None,
            });
        state.requests_remaining = Some(0);
        state.reset_at = Some(reset_at);
        state.source = QuotaSource::RetryAfter;
        state.updated_at = now;
    }

    /// Directly inject a full quota state (used by pollers like Gemini quota API).
    pub async fn update_state(&self, state: QuotaState) {
        let mut states = self.states.write().await;
        states.insert(state.provider.clone(), state);
    }

    /// Get the current state for a specific provider.
    pub async fn get_state(&self, provider: &str) -> Option<QuotaState> {
        self.states.read().await.get(provider).cloned()
    }

    /// Get all tracked provider states.
    pub async fn get_all_states(&self) -> Vec<QuotaState> {
        self.states.read().await.values().cloned().collect()
    }

    /// Non-blocking snapshot of all states (for sync rendering contexts like TUI).
    /// Returns empty vec if the lock is held.
    #[must_use]
    pub fn try_get_all_states(&self) -> Vec<QuotaState> {
        match self.states.try_read() {
            Ok(guard) => guard.values().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Duration until the provider's rate limit window resets.
    pub async fn time_until_reset(&self, provider: &str) -> Option<chrono::Duration> {
        let states = self.states.read().await;
        states
            .get(provider)
            .and_then(|s| s.reset_at)
            .map(|reset| reset - Utc::now())
            .filter(|d| d.num_seconds() > 0)
    }
}

impl Default for QuotaTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Singleton
// ============================================================================

lazy_static::lazy_static! {
    /// Global quota tracker instance
    static ref GLOBAL_QUOTA_TRACKER: Arc<QuotaTracker> = Arc::new(QuotaTracker::new());
}

/// Get the global quota tracker.
#[must_use]
pub fn global_quota_tracker() -> Arc<QuotaTracker> {
    Arc::clone(&GLOBAL_QUOTA_TRACKER)
}

// ============================================================================
// Header Helpers (pub(crate) for providers)
// ============================================================================

/// Extract a u64 value from a header.
pub(crate) fn header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}

/// Parse an ISO 8601 datetime header value.
pub(crate) fn header_datetime(headers: &HeaderMap, name: &str) -> Option<DateTime<Utc>> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

// ============================================================================
// Display Helpers
// ============================================================================

/// Format a number with K/M suffix for compact display.
#[must_use]
pub fn format_compact_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Format a chrono::Duration as human-readable "Xh Ym Zs".
#[must_use]
pub fn format_duration(d: &chrono::Duration) -> String {
    let total_secs = d.num_seconds();
    if total_secs <= 0 {
        return "now".to_string();
    }
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{}h {:02}m", hours, mins)
    } else if mins > 0 {
        format!("{}m {:02}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

#[cfg(test)]
mod tests;
