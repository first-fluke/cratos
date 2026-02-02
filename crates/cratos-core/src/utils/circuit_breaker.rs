//! Circuit Breaker pattern implementation
//!
//! Provides fault tolerance by preventing cascading failures.
//! The circuit breaker has three states:
//! - Closed: Normal operation, requests pass through
//! - Open: Failures exceeded threshold, requests are rejected
//! - HalfOpen: Testing if the service has recovered

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Failures exceeded threshold - requests are rejected
    Open,
    /// Testing recovery - limited requests pass through
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "Closed"),
            Self::Open => write!(f, "Open"),
            Self::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

/// Configuration for circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Number of successes in half-open state to close the circuit
    pub success_threshold: u32,
    /// Duration to wait before transitioning from open to half-open
    pub reset_timeout: Duration,
    /// Window size for counting failures (rolling window)
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            reset_timeout: Duration::from_secs(30),
            failure_window: Duration::from_secs(60),
        }
    }
}

impl CircuitBreakerConfig {
    /// Create a new configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set failure threshold
    #[must_use]
    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Set success threshold for half-open state
    #[must_use]
    pub fn with_success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }

    /// Set reset timeout
    #[must_use]
    pub fn with_reset_timeout(mut self, timeout: Duration) -> Self {
        self.reset_timeout = timeout;
        self
    }

    /// Set failure window
    #[must_use]
    pub fn with_failure_window(mut self, window: Duration) -> Self {
        self.failure_window = window;
        self
    }
}

/// Circuit breaker for fault tolerance
pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: RwLock<CircuitState>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: AtomicU64,
    opened_at: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    #[must_use]
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            config,
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure_time: AtomicU64::new(0),
            opened_at: AtomicU64::new(0),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults(name: impl Into<String>) -> Self {
        Self::new(name, CircuitBreakerConfig::default())
    }

    /// Get the circuit breaker name
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the current state
    #[must_use]
    pub fn state(&self) -> CircuitState {
        *self.state.read().unwrap()
    }

    /// Get current failure count
    #[must_use]
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::SeqCst)
    }

    /// Check if the circuit allows a request
    #[must_use]
    pub fn can_execute(&self) -> bool {
        self.check_state_transition();

        match self.state() {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful operation
    pub fn record_success(&self) {
        match self.state() {
            CircuitState::Closed => {
                // Reset failure count on success in closed state
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let successes = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                debug!(
                    name = %self.name,
                    successes = successes,
                    threshold = self.config.success_threshold,
                    "Circuit breaker success in half-open state"
                );

                if successes >= self.config.success_threshold {
                    self.close();
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but ignore
            }
        }
    }

    /// Record a failed operation
    pub fn record_failure(&self) {
        let now = current_timestamp();

        match self.state() {
            CircuitState::Closed => {
                // Check if we should reset the failure count (outside window)
                let last_failure = self.last_failure_time.load(Ordering::SeqCst);
                if last_failure > 0 {
                    let elapsed = Duration::from_millis(now - last_failure);
                    if elapsed > self.config.failure_window {
                        self.failure_count.store(0, Ordering::SeqCst);
                    }
                }

                self.last_failure_time.store(now, Ordering::SeqCst);
                let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;

                debug!(
                    name = %self.name,
                    failures = failures,
                    threshold = self.config.failure_threshold,
                    "Circuit breaker failure recorded"
                );

                if failures >= self.config.failure_threshold {
                    self.open();
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state reopens the circuit
                warn!(
                    name = %self.name,
                    "Circuit breaker failure in half-open state, reopening"
                );
                self.open();
            }
            CircuitState::Open => {
                // Already open, ignore
            }
        }
    }

    /// Check and perform state transitions
    fn check_state_transition(&self) {
        if self.state() == CircuitState::Open {
            let opened_at = self.opened_at.load(Ordering::SeqCst);
            let now = current_timestamp();
            let elapsed = Duration::from_millis(now - opened_at);

            if elapsed >= self.config.reset_timeout {
                self.half_open();
            }
        }
    }

    /// Transition to open state
    fn open(&self) {
        let mut state = self.state.write().unwrap();
        if *state != CircuitState::Open {
            info!(
                name = %self.name,
                failures = self.failure_count.load(Ordering::SeqCst),
                "Circuit breaker opened"
            );
            *state = CircuitState::Open;
            self.opened_at.store(current_timestamp(), Ordering::SeqCst);
        }
    }

    /// Transition to half-open state
    fn half_open(&self) {
        let mut state = self.state.write().unwrap();
        if *state == CircuitState::Open {
            info!(name = %self.name, "Circuit breaker entering half-open state");
            *state = CircuitState::HalfOpen;
            self.success_count.store(0, Ordering::SeqCst);
            self.failure_count.store(0, Ordering::SeqCst);
        }
    }

    /// Transition to closed state
    fn close(&self) {
        let mut state = self.state.write().unwrap();
        if *state != CircuitState::Closed {
            info!(name = %self.name, "Circuit breaker closed");
            *state = CircuitState::Closed;
            self.failure_count.store(0, Ordering::SeqCst);
            self.success_count.store(0, Ordering::SeqCst);
        }
    }

    /// Reset the circuit breaker to closed state
    pub fn reset(&self) {
        self.close();
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_config_defaults() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.success_threshold, 2);
        assert_eq!(config.reset_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_circuit_breaker_config_builder() {
        let config = CircuitBreakerConfig::new()
            .with_failure_threshold(10)
            .with_success_threshold(3)
            .with_reset_timeout(Duration::from_secs(60))
            .with_failure_window(Duration::from_secs(120));

        assert_eq!(config.failure_threshold, 10);
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.reset_timeout, Duration::from_secs(60));
        assert_eq!(config.failure_window, Duration::from_secs(120));
    }

    #[test]
    fn test_circuit_breaker_initial_state() {
        let cb = CircuitBreaker::with_defaults("test");
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.can_execute());
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig::new().with_failure_threshold(3);
        let cb = CircuitBreaker::new("test", config);

        // Record failures
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.can_execute());
    }

    #[test]
    fn test_circuit_breaker_success_resets_failures() {
        let config = CircuitBreakerConfig::new().with_failure_threshold(3);
        let cb = CircuitBreaker::new("test", config);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);

        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let config = CircuitBreakerConfig::new().with_failure_threshold(2);
        let cb = CircuitBreaker::new("test", config);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.can_execute());
    }

    #[test]
    fn test_circuit_state_display() {
        assert_eq!(format!("{}", CircuitState::Closed), "Closed");
        assert_eq!(format!("{}", CircuitState::Open), "Open");
        assert_eq!(format!("{}", CircuitState::HalfOpen), "HalfOpen");
    }
}
