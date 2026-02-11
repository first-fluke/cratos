//! Utility modules for cratos-core
//!
//! Provides common utilities:
//! - retry: Retry logic with exponential backoff
//! - circuit_breaker: Circuit breaker pattern for fault tolerance
//! - metrics: Lightweight metrics collection
//! - rate_limiter: Request rate limiting

mod circuit_breaker;
mod metrics;
mod rate_limiter;
mod retry;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use metrics::{
    global as metrics_global, Counter, Gauge, Histogram, LabeledCounter, LabeledHistogram,
    MetricsRegistry, Timer,
};
pub use rate_limiter::{RateLimitConfig, RateLimitResult, RateLimiter, TieredRateLimiter};
pub use retry::{retry_with_backoff, RetryConfig, RetryError};
