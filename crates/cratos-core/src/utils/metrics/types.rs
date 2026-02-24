use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// A thread-safe counter metric
#[derive(Debug, Default, Clone)]
pub struct Counter {
    value: Arc<AtomicU64>,
}

impl Counter {
    /// Create a new counter
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Increment the counter by 1
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment the counter by a specific amount
    pub fn inc_by(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Get the current value
    #[must_use]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset the counter to zero
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

/// A thread-safe gauge metric (can go up and down)
#[derive(Debug, Default, Clone)]
pub struct Gauge {
    value: Arc<AtomicI64>,
}

impl Gauge {
    /// Create a new gauge
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicI64::new(0)),
        }
    }

    /// Set the gauge value
    pub fn set(&self, value: i64) {
        self.value.store(value, Ordering::Relaxed);
    }

    /// Increment the gauge by 1
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the gauge by 1
    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get the current value
    #[must_use]
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// A histogram bucket
#[derive(Debug, Clone)]
pub struct HistogramBucket {
    pub bound: f64,
    pub count: Arc<AtomicU64>,
}

/// A histogram for tracking distributions
#[derive(Debug, Clone)]
pub struct Histogram {
    /// Upper-bound buckets for histogram distribution.
    pub buckets: Vec<HistogramBucket>,
    /// Running sum of all observed values (stored as u64 bits of f64).
    pub sum: Arc<AtomicU64>,
    /// Total number of observations.
    pub count: Arc<AtomicU64>,
}

impl Histogram {
    /// Create a histogram with default buckets suitable for latency (in ms)
    #[must_use]
    pub fn new() -> Self {
        Self::with_buckets(vec![
            5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
        ])
    }

    /// Create a histogram with custom buckets
    #[must_use]
    pub fn with_buckets(bucket_bounds: Vec<f64>) -> Self {
        let buckets = bucket_bounds
            .into_iter()
            .map(|b| HistogramBucket {
                bound: b,
                count: Arc::new(AtomicU64::new(0)),
            })
            .collect();

        Self {
            buckets,
            sum: Arc::new(AtomicU64::new(0)),
            count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Observe a value
    pub fn observe(&self, value: f64) {
        // Update sum (store as u64 bits with 3 decimal precision)
        let value_bits = (value * 1000.0) as u64;
        self.sum.fetch_add(value_bits, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);

        // Update buckets
        for bucket in &self.buckets {
            if value <= bucket.bound {
                bucket.count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get the count of observations
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get the sum of all observations
    #[must_use]
    pub fn sum(&self) -> f64 {
        self.sum.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get bucket counts
    #[must_use]
    pub fn bucket_counts(&self) -> Vec<(f64, u64)> {
        self.buckets
            .iter()
            .map(|b| (b.bound, b.count.load(Ordering::Relaxed)))
            .collect()
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer for measuring durations
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// Start a new timer
    #[must_use]
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed time in milliseconds
    #[must_use]
    pub fn elapsed_ms(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }

    /// Stop the timer and observe the duration in a histogram
    pub fn observe_duration(self, histogram: &Histogram) {
        histogram.observe(self.elapsed_ms());
    }
}
