//! Metrics collection for observability
//!
//! Provides lightweight metrics collection without external dependencies.
//! Metrics can be exported to Prometheus format or logged.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
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
struct HistogramBucket {
    bound: f64,
    count: Arc<AtomicU64>,
}

/// A histogram for tracking distributions
#[derive(Debug, Clone)]
pub struct Histogram {
    buckets: Vec<HistogramBucket>,
    sum: Arc<AtomicU64>,
    count: Arc<AtomicU64>,
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
    pub fn buckets(&self) -> Vec<(f64, u64)> {
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

/// Metrics registry for managing multiple metrics
#[derive(Default, Clone)]
pub struct MetricsRegistry {
    counters: Arc<RwLock<HashMap<String, Counter>>>,
    gauges: Arc<RwLock<HashMap<String, Gauge>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
}

impl MetricsRegistry {
    /// Create a new metrics registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a counter
    pub fn counter(&self, name: &str) -> Counter {
        let counters = self.counters.read().unwrap_or_else(|e| e.into_inner());
        if let Some(counter) = counters.get(name) {
            return counter.clone();
        }
        drop(counters);

        let mut counters = self.counters.write().unwrap_or_else(|e| e.into_inner());
        counters.entry(name.to_string()).or_default().clone()
    }

    /// Get or create a gauge
    pub fn gauge(&self, name: &str) -> Gauge {
        let gauges = self.gauges.read().unwrap_or_else(|e| e.into_inner());
        if let Some(gauge) = gauges.get(name) {
            return gauge.clone();
        }
        drop(gauges);

        let mut gauges = self.gauges.write().unwrap_or_else(|e| e.into_inner());
        gauges.entry(name.to_string()).or_default().clone()
    }

    /// Get or create a histogram
    pub fn histogram(&self, name: &str) -> Histogram {
        let histograms = self.histograms.read().unwrap_or_else(|e| e.into_inner());
        if let Some(histogram) = histograms.get(name) {
            return histogram.clone();
        }
        drop(histograms);

        let mut histograms = self.histograms.write().unwrap_or_else(|e| e.into_inner());
        histograms.entry(name.to_string()).or_default().clone()
    }

    /// Export metrics in Prometheus format
    #[must_use]
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Export counters
        let counters = self.counters.read().unwrap_or_else(|e| e.into_inner());
        for (name, counter) in counters.iter() {
            output.push_str(&format!(
                "# TYPE {} counter\n{} {}\n",
                name,
                name,
                counter.get()
            ));
        }

        // Export gauges
        let gauges = self.gauges.read().unwrap_or_else(|e| e.into_inner());
        for (name, gauge) in gauges.iter() {
            output.push_str(&format!(
                "# TYPE {} gauge\n{} {}\n",
                name,
                name,
                gauge.get()
            ));
        }

        // Export histograms
        let histograms = self.histograms.read().unwrap_or_else(|e| e.into_inner());
        for (name, histogram) in histograms.iter() {
            output.push_str(&format!("# TYPE {} histogram\n", name));
            for (bound, count) in histogram.buckets() {
                output.push_str(&format!("{}_bucket{{le=\"{}\"}} {}\n", name, bound, count));
            }
            output.push_str(&format!(
                "{}_bucket{{le=\"+Inf\"}} {}\n",
                name,
                histogram.count()
            ));
            output.push_str(&format!("{}_sum {}\n", name, histogram.sum()));
            output.push_str(&format!("{}_count {}\n", name, histogram.count()));
        }

        output
    }
}

/// Global metrics for the application
pub mod global {
    use super::*;
    use std::sync::OnceLock;

    static REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();

    /// Get the global metrics registry
    pub fn registry() -> &'static MetricsRegistry {
        REGISTRY.get_or_init(MetricsRegistry::new)
    }

    /// Convenience function to get a counter
    pub fn counter(name: &str) -> Counter {
        registry().counter(name)
    }

    /// Convenience function to get a gauge
    pub fn gauge(name: &str) -> Gauge {
        registry().gauge(name)
    }

    /// Convenience function to get a histogram
    pub fn histogram(name: &str) -> Histogram {
        registry().histogram(name)
    }

    /// Export all metrics in Prometheus format
    pub fn export_prometheus() -> String {
        registry().export_prometheus()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.inc_by(5);
        assert_eq!(counter.get(), 6);

        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_gauge() {
        let gauge = Gauge::new();
        assert_eq!(gauge.get(), 0);

        gauge.set(10);
        assert_eq!(gauge.get(), 10);

        gauge.inc();
        assert_eq!(gauge.get(), 11);

        gauge.dec();
        assert_eq!(gauge.get(), 10);
    }

    #[test]
    fn test_histogram() {
        let histogram = Histogram::with_buckets(vec![10.0, 50.0, 100.0]);

        histogram.observe(5.0);
        histogram.observe(25.0);
        histogram.observe(75.0);
        histogram.observe(150.0);

        assert_eq!(histogram.count(), 4);

        let buckets = histogram.buckets();
        assert_eq!(buckets[0], (10.0, 1)); // 5 <= 10
        assert_eq!(buckets[1], (50.0, 2)); // 5, 25 <= 50
        assert_eq!(buckets[2], (100.0, 3)); // 5, 25, 75 <= 100
    }

    #[test]
    fn test_timer() {
        let timer = Timer::start();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed_ms();
        assert!(elapsed >= 10.0);
    }

    #[test]
    fn test_metrics_registry() {
        let registry = MetricsRegistry::new();

        let counter1 = registry.counter("test_counter");
        counter1.inc();

        let counter2 = registry.counter("test_counter");
        assert_eq!(counter2.get(), 1);

        counter2.inc();
        assert_eq!(counter1.get(), 2);
    }

    #[test]
    fn test_prometheus_export() {
        let registry = MetricsRegistry::new();

        registry.counter("requests_total").inc_by(100);
        registry.gauge("active_connections").set(5);
        registry.histogram("request_duration_ms").observe(50.0);

        let output = registry.export_prometheus();

        assert!(output.contains("requests_total 100"));
        assert!(output.contains("active_connections 5"));
        assert!(output.contains("request_duration_ms_count 1"));
    }

    #[test]
    fn test_global_metrics() {
        global::counter("global_test").inc();
        assert_eq!(global::counter("global_test").get(), 1);
    }
}
