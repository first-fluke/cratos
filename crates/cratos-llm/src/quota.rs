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
        self.remaining_fraction
            .map(|f| f * 100.0)
            .or_else(|| {
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
struct PartialQuotaState {
    requests_remaining: Option<u64>,
    requests_limit: Option<u64>,
    tokens_remaining: Option<u64>,
    tokens_limit: Option<u64>,
    reset_at: Option<DateTime<Utc>>,
}

impl PartialQuotaState {
    /// Returns `true` if at least one field was parsed.
    fn has_data(&self) -> bool {
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
        let state = states.entry(provider.to_string()).or_insert_with(|| QuotaState {
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
        let state = states.entry(provider.to_string()).or_insert_with(|| QuotaState {
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
// Header Parsers
// ============================================================================

/// Parse Anthropic rate limit headers.
///
/// Headers:
/// - `anthropic-ratelimit-requests-limit`
/// - `anthropic-ratelimit-requests-remaining`
/// - `anthropic-ratelimit-requests-reset`
/// - `anthropic-ratelimit-tokens-limit`
/// - `anthropic-ratelimit-tokens-remaining`
/// - `anthropic-ratelimit-tokens-reset`
fn parse_anthropic_headers(headers: &HeaderMap) -> PartialQuotaState {
    PartialQuotaState {
        requests_limit: header_u64(headers, "anthropic-ratelimit-requests-limit"),
        requests_remaining: header_u64(headers, "anthropic-ratelimit-requests-remaining"),
        tokens_limit: header_u64(headers, "anthropic-ratelimit-tokens-limit"),
        tokens_remaining: header_u64(headers, "anthropic-ratelimit-tokens-remaining"),
        // Anthropic uses ISO 8601 for reset times
        reset_at: header_datetime(headers, "anthropic-ratelimit-requests-reset")
            .or_else(|| header_datetime(headers, "anthropic-ratelimit-tokens-reset")),
    }
}

/// Parse OpenAI-compatible rate limit headers.
///
/// Headers:
/// - `x-ratelimit-limit-requests`
/// - `x-ratelimit-remaining-requests`
/// - `x-ratelimit-reset-requests`
/// - `x-ratelimit-limit-tokens`
/// - `x-ratelimit-remaining-tokens`
/// - `x-ratelimit-reset-tokens`
fn parse_openai_headers(headers: &HeaderMap) -> PartialQuotaState {
    PartialQuotaState {
        requests_limit: header_u64(headers, "x-ratelimit-limit-requests"),
        requests_remaining: header_u64(headers, "x-ratelimit-remaining-requests"),
        tokens_limit: header_u64(headers, "x-ratelimit-limit-tokens"),
        tokens_remaining: header_u64(headers, "x-ratelimit-remaining-tokens"),
        // OpenAI uses durations like "6m0s" or ISO 8601 timestamps
        reset_at: header_openai_reset(headers, "x-ratelimit-reset-requests")
            .or_else(|| header_openai_reset(headers, "x-ratelimit-reset-tokens")),
    }
}

/// Fallback parser that tries both header formats.
fn parse_generic_headers(headers: &HeaderMap) -> PartialQuotaState {
    let anthropic = parse_anthropic_headers(headers);
    if anthropic.has_data() {
        return anthropic;
    }
    parse_openai_headers(headers)
}

// ============================================================================
// Header Helpers
// ============================================================================

/// Extract a u64 value from a header.
fn header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}

/// Parse an ISO 8601 datetime header value.
fn header_datetime(headers: &HeaderMap, name: &str) -> Option<DateTime<Utc>> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

/// Parse OpenAI-style reset header.
///
/// OpenAI returns durations like `"6m0s"`, `"1m30.5s"`, `"200ms"`,
/// or occasionally ISO 8601 timestamps.
fn header_openai_reset(headers: &HeaderMap, name: &str) -> Option<DateTime<Utc>> {
    let val = headers.get(name).and_then(|v| v.to_str().ok())?;

    // Try ISO 8601 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(val) {
        return Some(dt.with_timezone(&Utc));
    }

    // Parse Go-style duration: "6m0s", "1m30.5s", "200ms", "45s"
    if let Some(secs) = parse_go_duration(val) {
        return Some(Utc::now() + chrono::Duration::milliseconds((secs * 1000.0) as i64));
    }

    None
}

/// Parse a Go-style duration string into total seconds.
///
/// Supports patterns: `"6m0s"`, `"1m30.5s"`, `"200ms"`, `"45s"`, `"1h2m3s"`.
fn parse_go_duration(s: &str) -> Option<f64> {
    let mut total_secs = 0.0_f64;
    let mut num_buf = String::new();
    let mut chars = s.chars().peekable();
    let mut parsed_any = false;

    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() || ch == '.' {
            num_buf.push(ch);
            chars.next();
        } else if ch == 'h' {
            chars.next();
            let val: f64 = num_buf.parse().ok()?;
            total_secs += val * 3600.0;
            num_buf.clear();
            parsed_any = true;
        } else if ch == 'm' {
            chars.next();
            // Check for "ms"
            if chars.peek() == Some(&'s') {
                chars.next();
                let val: f64 = num_buf.parse().ok()?;
                total_secs += val / 1000.0;
            } else {
                let val: f64 = num_buf.parse().ok()?;
                total_secs += val * 60.0;
            }
            num_buf.clear();
            parsed_any = true;
        } else if ch == 's' {
            chars.next();
            let val: f64 = num_buf.parse().ok()?;
            total_secs += val;
            num_buf.clear();
            parsed_any = true;
        } else {
            // Unknown character
            return None;
        }
    }

    if parsed_any { Some(total_secs) } else { None }
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn test_parse_go_duration_minutes_seconds() {
        assert!((parse_go_duration("6m0s").unwrap() - 360.0).abs() < 0.001);
        assert!((parse_go_duration("1m30s").unwrap() - 90.0).abs() < 0.001);
        assert!((parse_go_duration("1m30.5s").unwrap() - 90.5).abs() < 0.001);
    }

    #[test]
    fn test_parse_go_duration_milliseconds() {
        assert!((parse_go_duration("200ms").unwrap() - 0.2).abs() < 0.001);
        assert!((parse_go_duration("1500ms").unwrap() - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_parse_go_duration_seconds() {
        assert!((parse_go_duration("45s").unwrap() - 45.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_go_duration_complex() {
        assert!((parse_go_duration("1h2m3s").unwrap() - 3723.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_go_duration_invalid() {
        assert!(parse_go_duration("abc").is_none());
        assert!(parse_go_duration("").is_none());
    }

    #[test]
    fn test_parse_anthropic_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "anthropic-ratelimit-requests-limit",
            HeaderValue::from_static("1000"),
        );
        headers.insert(
            "anthropic-ratelimit-requests-remaining",
            HeaderValue::from_static("987"),
        );
        headers.insert(
            "anthropic-ratelimit-tokens-limit",
            HeaderValue::from_static("100000"),
        );
        headers.insert(
            "anthropic-ratelimit-tokens-remaining",
            HeaderValue::from_static("45200"),
        );
        headers.insert(
            "anthropic-ratelimit-requests-reset",
            HeaderValue::from_static("2026-02-06T15:30:00Z"),
        );

        let partial = parse_anthropic_headers(&headers);
        assert_eq!(partial.requests_limit, Some(1000));
        assert_eq!(partial.requests_remaining, Some(987));
        assert_eq!(partial.tokens_limit, Some(100_000));
        assert_eq!(partial.tokens_remaining, Some(45_200));
        assert!(partial.reset_at.is_some());
    }

    #[test]
    fn test_parse_openai_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("60"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("12"),
        );
        headers.insert(
            "x-ratelimit-limit-tokens",
            HeaderValue::from_static("15000"),
        );
        headers.insert(
            "x-ratelimit-remaining-tokens",
            HeaderValue::from_static("8500"),
        );
        headers.insert(
            "x-ratelimit-reset-requests",
            HeaderValue::from_static("2m14s"),
        );

        let partial = parse_openai_headers(&headers);
        assert_eq!(partial.requests_limit, Some(60));
        assert_eq!(partial.requests_remaining, Some(12));
        assert_eq!(partial.tokens_limit, Some(15_000));
        assert_eq!(partial.tokens_remaining, Some(8_500));
        assert!(partial.reset_at.is_some());
    }

    #[test]
    fn test_parse_empty_headers() {
        let headers = HeaderMap::new();
        let partial = parse_anthropic_headers(&headers);
        assert!(!partial.has_data());
    }

    #[tokio::test]
    async fn test_quota_tracker_update_and_get() {
        let tracker = QuotaTracker::new();

        let mut headers = HeaderMap::new();
        headers.insert(
            "anthropic-ratelimit-requests-limit",
            HeaderValue::from_static("1000"),
        );
        headers.insert(
            "anthropic-ratelimit-requests-remaining",
            HeaderValue::from_static("999"),
        );

        tracker.update_from_headers("anthropic", &headers).await;

        let state = tracker.get_state("anthropic").await.unwrap();
        assert_eq!(state.requests_limit, Some(1000));
        assert_eq!(state.requests_remaining, Some(999));
    }

    #[tokio::test]
    async fn test_quota_tracker_incremental_merge() {
        let tracker = QuotaTracker::new();

        // First update: requests only
        let mut h1 = HeaderMap::new();
        h1.insert(
            "anthropic-ratelimit-requests-limit",
            HeaderValue::from_static("1000"),
        );
        h1.insert(
            "anthropic-ratelimit-requests-remaining",
            HeaderValue::from_static("999"),
        );
        tracker.update_from_headers("anthropic", &h1).await;

        // Second update: tokens only
        let mut h2 = HeaderMap::new();
        h2.insert(
            "anthropic-ratelimit-tokens-limit",
            HeaderValue::from_static("100000"),
        );
        h2.insert(
            "anthropic-ratelimit-tokens-remaining",
            HeaderValue::from_static("50000"),
        );
        tracker.update_from_headers("anthropic", &h2).await;

        let state = tracker.get_state("anthropic").await.unwrap();
        assert_eq!(state.requests_limit, Some(1000));
        assert_eq!(state.tokens_limit, Some(100_000));
    }

    #[tokio::test]
    async fn test_quota_tracker_get_all() {
        let tracker = QuotaTracker::new();

        let mut h = HeaderMap::new();
        h.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("60"),
        );
        h.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("30"),
        );

        tracker.update_from_headers("groq", &h).await;
        tracker.update_from_headers("deepseek", &h).await;

        let all = tracker.get_all_states().await;
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_quota_state_usage_pct() {
        let state = QuotaState {
            provider: "test".to_string(),
            requests_remaining: Some(100),
            requests_limit: Some(1000),
            tokens_remaining: Some(50_000),
            tokens_limit: Some(100_000),
            reset_at: None,
            updated_at: Utc::now(),
            source: QuotaSource::ResponseHeaders,
            remaining_fraction: None,
            tier_label: None,
        };
        assert!((state.requests_usage_pct().unwrap() - 90.0).abs() < 0.01);
        assert!((state.tokens_usage_pct().unwrap() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_is_near_limit() {
        let state = QuotaState {
            provider: "test".to_string(),
            requests_remaining: Some(5),
            requests_limit: Some(100),
            tokens_remaining: Some(90_000),
            tokens_limit: Some(100_000),
            reset_at: None,
            updated_at: Utc::now(),
            source: QuotaSource::ResponseHeaders,
            remaining_fraction: None,
            tier_label: None,
        };
        // 95% usage, threshold 20% -> near limit
        assert!(state.is_near_limit(20.0));
    }

    #[test]
    fn test_format_compact_number() {
        assert_eq!(format_compact_number(500), "500");
        assert_eq!(format_compact_number(1_500), "1.5K");
        assert_eq!(format_compact_number(45_200), "45.2K");
        assert_eq!(format_compact_number(1_500_000), "1.5M");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(&chrono::Duration::seconds(45)), "45s");
        assert_eq!(format_duration(&chrono::Duration::seconds(134)), "2m 14s");
        assert_eq!(format_duration(&chrono::Duration::seconds(3723)), "1h 02m");
        assert_eq!(format_duration(&chrono::Duration::seconds(0)), "now");
    }

    #[tokio::test]
    async fn test_update_from_retry_after() {
        let tracker = QuotaTracker::new();
        tracker.update_from_retry_after("gemini", 30).await;

        let state = tracker.get_state("gemini").await.unwrap();
        assert_eq!(state.requests_remaining, Some(0));
        assert!(state.reset_at.is_some());
        assert_eq!(state.source, QuotaSource::RetryAfter);
    }

    #[test]
    fn test_remaining_pct_from_fraction() {
        let state = QuotaState {
            provider: "gemini".to_string(),
            requests_remaining: None,
            requests_limit: None,
            tokens_remaining: None,
            tokens_limit: None,
            reset_at: None,
            updated_at: Utc::now(),
            source: QuotaSource::PolledApi,
            remaining_fraction: Some(0.85),
            tier_label: None,
        };
        let pct = state.remaining_pct().unwrap();
        assert!((pct - 85.0).abs() < 0.01);
    }

    #[test]
    fn test_remaining_pct_from_requests() {
        let state = QuotaState {
            provider: "anthropic".to_string(),
            requests_remaining: Some(750),
            requests_limit: Some(1000),
            tokens_remaining: None,
            tokens_limit: None,
            reset_at: None,
            updated_at: Utc::now(),
            source: QuotaSource::ResponseHeaders,
            remaining_fraction: None,
            tier_label: None,
        };
        let pct = state.remaining_pct().unwrap();
        assert!((pct - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_remaining_pct_none() {
        let state = QuotaState {
            provider: "unknown".to_string(),
            requests_remaining: None,
            requests_limit: None,
            tokens_remaining: None,
            tokens_limit: None,
            reset_at: None,
            updated_at: Utc::now(),
            source: QuotaSource::ResponseHeaders,
            remaining_fraction: None,
            tier_label: None,
        };
        assert!(state.remaining_pct().is_none());
    }

    #[tokio::test]
    async fn test_update_state_direct() {
        let tracker = QuotaTracker::new();
        let state = QuotaState {
            provider: "gemini".to_string(),
            requests_remaining: None,
            requests_limit: None,
            tokens_remaining: None,
            tokens_limit: None,
            reset_at: None,
            updated_at: Utc::now(),
            source: QuotaSource::PolledApi,
            remaining_fraction: Some(0.42),
            tier_label: Some("free".to_string()),
        };
        tracker.update_state(state).await;

        let retrieved = tracker.get_state("gemini").await.unwrap();
        assert_eq!(retrieved.source, QuotaSource::PolledApi);
        assert!((retrieved.remaining_fraction.unwrap() - 0.42).abs() < 0.001);
        assert_eq!(retrieved.tier_label.as_deref(), Some("free"));
    }
}
