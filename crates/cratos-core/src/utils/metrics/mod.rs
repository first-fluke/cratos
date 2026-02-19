//! Metrics collection for observability
//!
//! Provides lightweight metrics collection without external dependencies.
//! Metrics can be exported to Prometheus format or logged.

pub mod labeled;
pub mod registry;
pub mod types;

pub use labeled::{LabeledCounter, LabeledHistogram};
pub use registry::MetricsRegistry;
pub use types::{Counter, Gauge, Histogram, Timer};

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

    /// Convenience function to get a labeled counter
    pub fn labeled_counter(name: &str) -> LabeledCounter {
        registry().labeled_counter(name)
    }

    /// Convenience function to get a labeled histogram
    pub fn labeled_histogram(name: &str) -> LabeledHistogram {
        registry().labeled_histogram(name)
    }

    /// Export all metrics in Prometheus format
    pub fn export_prometheus() -> String {
        registry().export_prometheus()
    }
}

#[cfg(test)]
mod tests;
