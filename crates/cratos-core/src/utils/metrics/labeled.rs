use super::types::{Counter, Histogram};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Label key — a sorted vector of (key, value) pairs.
pub type LabelKey = Vec<(String, String)>;

/// A labeled counter — maintains separate counters per label set.
#[derive(Debug, Default, Clone)]
pub struct LabeledCounter {
    entries: Arc<RwLock<HashMap<LabelKey, Counter>>>,
}

impl LabeledCounter {
    /// Create a new labeled counter
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment by 1 for the given label set
    pub fn inc(&self, labels: &[(&str, &str)]) {
        self.inc_by(labels, 1);
    }

    /// Increment by `n` for the given label set
    pub fn inc_by(&self, labels: &[(&str, &str)], n: u64) {
        let key: LabelKey = labels
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();

        let counters = self.entries.read().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = counters.get(&key) {
            c.inc_by(n);
            return;
        }
        drop(counters);

        let mut counters = self.entries.write().unwrap_or_else(|e| e.into_inner());
        let counter = counters.entry(key).or_default();
        counter.inc_by(n);
    }

    /// Get all entries (label set → value)
    #[must_use]
    pub fn entries(&self) -> Vec<(LabelKey, u64)> {
        let counters = self.entries.read().unwrap_or_else(|e| e.into_inner());
        counters
            .iter()
            .map(|(labels, c)| (labels.clone(), c.get()))
            .collect()
    }
}

/// A labeled histogram — maintains separate histograms per label set.
#[derive(Debug, Clone)]
pub struct LabeledHistogram {
    entries: Arc<RwLock<HashMap<LabelKey, Histogram>>>,
    bucket_bounds: Vec<f64>,
}

impl Default for LabeledHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl LabeledHistogram {
    /// Create with default latency buckets (seconds)
    #[must_use]
    pub fn new() -> Self {
        Self::with_buckets(vec![
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ])
    }

    /// Create with custom bucket bounds
    #[must_use]
    pub fn with_buckets(bucket_bounds: Vec<f64>) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            bucket_bounds,
        }
    }

    /// Observe a value for the given label set
    pub fn observe(&self, labels: &[(&str, &str)], value: f64) {
        let key: LabelKey = labels
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();

        let histograms = self.entries.read().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = histograms.get(&key) {
            h.observe(value);
            return;
        }
        drop(histograms);

        let mut histograms = self.entries.write().unwrap_or_else(|e| e.into_inner());
        let histogram = histograms
            .entry(key)
            .or_insert_with(|| Histogram::with_buckets(self.bucket_bounds.clone()));
        histogram.observe(value);
    }

    /// Get all entries (label set → histogram)
    #[must_use]
    pub fn entries(&self) -> Vec<(LabelKey, Histogram)> {
        let histograms = self.entries.read().unwrap_or_else(|e| e.into_inner());
        histograms
            .iter()
            .map(|(labels, h)| (labels.clone(), h.clone()))
            .collect()
    }
}

/// Format label pairs as Prometheus label string: `{key1="val1",key2="val2"}`
pub fn format_labels(labels: &[(String, String)]) -> String {
    if labels.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = labels
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", k, v))
        .collect();
    format!("{{{}}}", parts.join(","))
}
