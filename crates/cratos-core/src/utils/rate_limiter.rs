//! Rate limiting for request throttling
//!
//! Provides token bucket and sliding window rate limiters.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests allowed in the window
    pub max_requests: u32,
    /// Time window for rate limiting
    pub window: Duration,
    /// Whether to use sliding window (vs fixed window)
    pub sliding: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 60,
            window: Duration::from_secs(60),
            sliding: true,
        }
    }
}

impl RateLimitConfig {
    /// Create a new rate limit config
    #[must_use]
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            sliding: true,
        }
    }

    /// Set whether to use sliding window
    #[must_use]
    pub fn with_sliding(mut self, sliding: bool) -> Self {
        self.sliding = sliding;
        self
    }

    /// Create config for requests per second
    #[must_use]
    pub fn per_second(max_requests: u32) -> Self {
        Self::new(max_requests, Duration::from_secs(1))
    }

    /// Create config for requests per minute
    #[must_use]
    pub fn per_minute(max_requests: u32) -> Self {
        Self::new(max_requests, Duration::from_secs(60))
    }

    /// Create config for requests per hour
    #[must_use]
    pub fn per_hour(max_requests: u32) -> Self {
        Self::new(max_requests, Duration::from_secs(3600))
    }
}

/// Result of a rate limit check
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Remaining requests in the current window
    pub remaining: u32,
    /// Time until the limit resets
    pub reset_after: Duration,
    /// Current request count
    pub current: u32,
}

impl RateLimitResult {
    /// Create an allowed result
    fn allowed(remaining: u32, reset_after: Duration, current: u32) -> Self {
        Self {
            allowed: true,
            remaining,
            reset_after,
            current,
        }
    }

    /// Create a denied result
    fn denied(reset_after: Duration, current: u32, max: u32) -> Self {
        Self {
            allowed: false,
            remaining: 0,
            reset_after,
            current: current.min(max),
        }
    }
}

/// Request timestamp for sliding window
#[derive(Debug, Clone)]
struct RequestRecord {
    timestamp: Instant,
}

/// In-memory rate limiter using sliding window algorithm
#[derive(Debug)]
pub struct RateLimiter {
    config: RateLimitConfig,
    /// Map of key -> request timestamps
    requests: Arc<RwLock<HashMap<String, Vec<RequestRecord>>>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    #[must_use]
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if a request is allowed for the given key
    pub async fn check(&self, key: &str) -> RateLimitResult {
        let now = Instant::now();
        let window_start = now - self.config.window;

        let requests = self.requests.read().await;
        let records = requests.get(key);

        let current_count = match records {
            Some(records) => {
                if self.config.sliding {
                    // Count requests within the window
                    records
                        .iter()
                        .filter(|r| r.timestamp > window_start)
                        .count() as u32
                } else {
                    records.len() as u32
                }
            }
            None => 0,
        };

        let remaining = self.config.max_requests.saturating_sub(current_count);
        let reset_after = self.calculate_reset_after(records, now);

        if current_count < self.config.max_requests {
            RateLimitResult::allowed(remaining.saturating_sub(1), reset_after, current_count + 1)
        } else {
            RateLimitResult::denied(reset_after, current_count, self.config.max_requests)
        }
    }

    /// Record a request for the given key
    pub async fn record(&self, key: &str) {
        let now = Instant::now();
        let window_start = now - self.config.window;

        let mut requests = self.requests.write().await;
        let records = requests.entry(key.to_string()).or_default();

        // Clean up old records if using sliding window
        if self.config.sliding {
            records.retain(|r| r.timestamp > window_start);
        }

        records.push(RequestRecord { timestamp: now });
    }

    /// Check and record in one operation (returns whether allowed)
    pub async fn acquire(&self, key: &str) -> RateLimitResult {
        let result = self.check(key).await;
        if result.allowed {
            self.record(key).await;
        }
        result
    }

    /// Get current usage for a key
    pub async fn usage(&self, key: &str) -> (u32, u32) {
        let now = Instant::now();
        let window_start = now - self.config.window;

        let requests = self.requests.read().await;
        let current = match requests.get(key) {
            Some(records) => {
                if self.config.sliding {
                    records
                        .iter()
                        .filter(|r| r.timestamp > window_start)
                        .count() as u32
                } else {
                    records.len() as u32
                }
            }
            None => 0,
        };

        (current, self.config.max_requests)
    }

    /// Reset rate limit for a key
    pub async fn reset(&self, key: &str) {
        let mut requests = self.requests.write().await;
        requests.remove(key);
    }

    /// Clean up expired entries
    pub async fn cleanup(&self) -> usize {
        let now = Instant::now();
        let window_start = now - self.config.window;

        let mut requests = self.requests.write().await;
        let initial_count = requests.len();

        // Remove entries with no recent requests
        requests.retain(|_, records| {
            records.retain(|r| r.timestamp > window_start);
            !records.is_empty()
        });

        initial_count - requests.len()
    }

    fn calculate_reset_after(
        &self,
        records: Option<&Vec<RequestRecord>>,
        now: Instant,
    ) -> Duration {
        match records {
            Some(records) if !records.is_empty() => {
                if self.config.sliding {
                    // Find oldest request in window
                    let window_start = now - self.config.window;
                    if let Some(oldest) = records
                        .iter()
                        .filter(|r| r.timestamp > window_start)
                        .min_by_key(|r| r.timestamp)
                    {
                        let elapsed = now.duration_since(oldest.timestamp);
                        self.config.window.saturating_sub(elapsed)
                    } else {
                        Duration::ZERO
                    }
                } else {
                    // Fixed window - find when first request was made
                    if let Some(first) = records.first() {
                        let elapsed = now.duration_since(first.timestamp);
                        self.config.window.saturating_sub(elapsed)
                    } else {
                        Duration::ZERO
                    }
                }
            }
            _ => Duration::ZERO,
        }
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            requests: Arc::clone(&self.requests),
        }
    }
}

/// Multi-tier rate limiter for different limit levels
#[derive(Debug, Clone)]
pub struct TieredRateLimiter {
    /// Per-user limits
    user_limiter: RateLimiter,
    /// Global limits
    global_limiter: RateLimiter,
}

impl TieredRateLimiter {
    /// Create a new tiered rate limiter
    #[must_use]
    pub fn new(user_config: RateLimitConfig, global_config: RateLimitConfig) -> Self {
        Self {
            user_limiter: RateLimiter::new(user_config),
            global_limiter: RateLimiter::new(global_config),
        }
    }

    /// Check if a request is allowed (checks both user and global limits)
    pub async fn acquire(&self, user_key: &str) -> RateLimitResult {
        // Check global limit first
        let global_result = self.global_limiter.acquire("global").await;
        if !global_result.allowed {
            return global_result;
        }

        // Then check user limit
        self.user_limiter.acquire(user_key).await
    }

    /// Get usage for a user
    pub async fn user_usage(&self, user_key: &str) -> (u32, u32) {
        self.user_limiter.usage(user_key).await
    }

    /// Get global usage
    pub async fn global_usage(&self) -> (u32, u32) {
        self.global_limiter.usage("global").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_under_limit() {
        let limiter = RateLimiter::new(RateLimitConfig::new(5, Duration::from_secs(60)));

        for _ in 0..5 {
            let result = limiter.acquire("user1").await;
            assert!(result.allowed);
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_denies_over_limit() {
        let limiter = RateLimiter::new(RateLimitConfig::new(3, Duration::from_secs(60)));

        // Use up the limit
        for _ in 0..3 {
            let result = limiter.acquire("user1").await;
            assert!(result.allowed);
        }

        // Should be denied
        let result = limiter.acquire("user1").await;
        assert!(!result.allowed);
        assert_eq!(result.remaining, 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_separate_keys() {
        let limiter = RateLimiter::new(RateLimitConfig::new(2, Duration::from_secs(60)));

        // User 1 uses their limit
        limiter.acquire("user1").await;
        limiter.acquire("user1").await;
        let result1 = limiter.acquire("user1").await;
        assert!(!result1.allowed);

        // User 2 should still have their limit
        let result2 = limiter.acquire("user2").await;
        assert!(result2.allowed);
    }

    #[tokio::test]
    async fn test_rate_limiter_check_without_record() {
        let limiter = RateLimiter::new(RateLimitConfig::new(5, Duration::from_secs(60)));

        // Check doesn't consume - remaining shows what would be left after this request
        let result1 = limiter.check("user1").await;
        assert!(result1.allowed);
        assert_eq!(result1.remaining, 4); // 5 - 1 = 4 would remain after this request

        let result2 = limiter.check("user1").await;
        assert!(result2.allowed);
        assert_eq!(result2.remaining, 4); // Still 4 since check doesn't record

        // Record consumes
        limiter.record("user1").await;
        let result3 = limiter.check("user1").await;
        assert!(result3.allowed);
        assert_eq!(result3.current, 2); // Will be 2 after this request (1 recorded + 1 new)
        assert_eq!(result3.remaining, 3); // 5 - 2 = 3 would remain
    }

    #[tokio::test]
    async fn test_rate_limiter_usage() {
        let limiter = RateLimiter::new(RateLimitConfig::new(10, Duration::from_secs(60)));

        let (current, max) = limiter.usage("user1").await;
        assert_eq!(current, 0);
        assert_eq!(max, 10);

        limiter.acquire("user1").await;
        limiter.acquire("user1").await;

        let (current, max) = limiter.usage("user1").await;
        assert_eq!(current, 2);
        assert_eq!(max, 10);
    }

    #[tokio::test]
    async fn test_rate_limiter_reset() {
        let limiter = RateLimiter::new(RateLimitConfig::new(2, Duration::from_secs(60)));

        limiter.acquire("user1").await;
        limiter.acquire("user1").await;

        let result = limiter.acquire("user1").await;
        assert!(!result.allowed);

        // Reset
        limiter.reset("user1").await;

        let result = limiter.acquire("user1").await;
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_tiered_rate_limiter() {
        let tiered = TieredRateLimiter::new(
            RateLimitConfig::new(5, Duration::from_secs(60)), // Per user
            RateLimitConfig::new(10, Duration::from_secs(60)), // Global
        );

        // User limit should apply
        for _ in 0..5 {
            let result = tiered.acquire("user1").await;
            assert!(result.allowed);
        }

        let result = tiered.acquire("user1").await;
        assert!(!result.allowed);

        // Different user should still work (global not exhausted)
        let result = tiered.acquire("user2").await;
        assert!(result.allowed);
    }

    #[test]
    fn test_config_builders() {
        let per_sec = RateLimitConfig::per_second(10);
        assert_eq!(per_sec.max_requests, 10);
        assert_eq!(per_sec.window, Duration::from_secs(1));

        let per_min = RateLimitConfig::per_minute(60);
        assert_eq!(per_min.max_requests, 60);
        assert_eq!(per_min.window, Duration::from_secs(60));

        let per_hour = RateLimitConfig::per_hour(1000);
        assert_eq!(per_hour.max_requests, 1000);
        assert_eq!(per_hour.window, Duration::from_secs(3600));
    }
}
