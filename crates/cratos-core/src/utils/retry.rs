//! Retry logic with exponential backoff
//!
//! Provides configurable retry behavior for transient failures.

use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Add random jitter to delays
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Create a new retry configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum attempts
    #[must_use]
    pub fn with_max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Set initial delay
    #[must_use]
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set maximum delay
    #[must_use]
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set backoff multiplier
    #[must_use]
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Enable or disable jitter
    #[must_use]
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// Calculate delay for a given attempt number
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32 - 1);

        let delay_ms = base_delay.min(self.max_delay.as_millis() as f64) as u64;

        let final_delay = if self.jitter {
            // Add up to 25% jitter
            let jitter_range = delay_ms / 4;
            let jitter = rand_jitter(jitter_range);
            delay_ms + jitter
        } else {
            delay_ms
        };

        Duration::from_millis(final_delay)
    }
}

/// Simple pseudo-random jitter (avoid adding rand crate dependency)
fn rand_jitter(max: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    // Use current time nanoseconds as simple randomness source
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    nanos % max
}

/// Error type for retry operations
#[derive(Debug)]
pub struct RetryError<E> {
    /// The last error encountered
    pub last_error: E,
    /// Total number of attempts made
    pub attempts: u32,
}

impl<E: std::fmt::Display> std::fmt::Display for RetryError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Operation failed after {} attempts: {}",
            self.attempts, self.last_error
        )
    }
}

impl<E: std::fmt::Debug + std::fmt::Display> std::error::Error for RetryError<E> {}

/// Execute an async operation with retry logic
///
/// # Arguments
/// * `config` - Retry configuration
/// * `operation` - Async operation to retry
/// * `is_retryable` - Function to determine if an error should trigger a retry
///
/// # Example
/// ```ignore
/// let config = RetryConfig::default();
/// let result = retry_with_backoff(
///     &config,
///     || async { make_http_request().await },
///     |e| e.is_transient(),
/// ).await;
/// ```
pub async fn retry_with_backoff<T, E, F, Fut, R>(
    config: &RetryConfig,
    mut operation: F,
    is_retryable: R,
) -> Result<T, RetryError<E>>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    R: Fn(&E) -> bool,
    E: std::fmt::Debug,
{
    for attempt in 1..=config.max_attempts {
        match operation().await {
            Ok(result) => {
                if attempt > 1 {
                    debug!(attempt = attempt, "Operation succeeded after retry");
                }
                return Ok(result);
            }
            Err(e) => {
                let is_retryable_err = is_retryable(&e);
                let should_retry = attempt < config.max_attempts && is_retryable_err;

                if should_retry {
                    let delay = config.calculate_delay(attempt);
                    warn!(
                        attempt = attempt,
                        max_attempts = config.max_attempts,
                        delay_ms = delay.as_millis() as u64,
                        error = ?e,
                        "Operation failed, retrying"
                    );
                    sleep(delay).await;
                } else {
                    debug!(
                        attempt = attempt,
                        error = ?e,
                        "Operation failed, no more retries"
                    );
                    // Return immediately if error is not retryable or max attempts reached
                    return Err(RetryError {
                        last_error: e,
                        attempts: attempt,
                    });
                }
            }
        }
    }

    // This should be unreachable since we always return from the Err branch
    unreachable!("retry loop should always return from error handling")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_retry_config_builder() {
        let config = RetryConfig::new()
            .with_max_attempts(5)
            .with_initial_delay(Duration::from_millis(200))
            .with_max_delay(Duration::from_secs(30))
            .with_backoff_multiplier(3.0)
            .with_jitter(false);

        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_delay, Duration::from_millis(200));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 3.0);
        assert!(!config.jitter);
    }

    #[test]
    fn test_calculate_delay() {
        let config = RetryConfig::new()
            .with_initial_delay(Duration::from_millis(100))
            .with_backoff_multiplier(2.0)
            .with_jitter(false);

        assert_eq!(config.calculate_delay(1), Duration::from_millis(100));
        assert_eq!(config.calculate_delay(2), Duration::from_millis(200));
        assert_eq!(config.calculate_delay(3), Duration::from_millis(400));
    }

    #[test]
    fn test_delay_respects_max() {
        let config = RetryConfig::new()
            .with_initial_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(5))
            .with_backoff_multiplier(10.0)
            .with_jitter(false);

        // 1 * 10^2 = 100 seconds, but max is 5 seconds
        assert_eq!(config.calculate_delay(3), Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let config = RetryConfig::new().with_max_attempts(3);
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32, RetryError<&str>> = retry_with_backoff(
            &config,
            || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<i32, &str>(42)
                }
            },
            |_| true,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let config = RetryConfig::new()
            .with_max_attempts(3)
            .with_initial_delay(Duration::from_millis(1));

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32, RetryError<&str>> = retry_with_backoff(
            &config,
            || {
                let c = counter_clone.clone();
                async move {
                    let count = c.fetch_add(1, Ordering::SeqCst);
                    if count < 2 {
                        Err("transient error")
                    } else {
                        Ok(42)
                    }
                }
            },
            |_| true,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_all_attempts_fail() {
        let config = RetryConfig::new()
            .with_max_attempts(3)
            .with_initial_delay(Duration::from_millis(1));

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32, RetryError<&str>> = retry_with_backoff(
            &config,
            || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, &str>("persistent error")
                }
            },
            |_| true,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.attempts, 3);
        assert_eq!(err.last_error, "persistent error");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let config = RetryConfig::new()
            .with_max_attempts(3)
            .with_initial_delay(Duration::from_millis(1));

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32, RetryError<&str>> = retry_with_backoff(
            &config,
            || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, &str>("non-retryable error")
                }
            },
            |_| false, // Never retry
        )
        .await;

        assert!(result.is_err());
        // Should only attempt once since error is not retryable
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
