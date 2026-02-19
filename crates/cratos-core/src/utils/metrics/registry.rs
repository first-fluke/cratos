use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use super::types::{Counter, Gauge, Histogram};
use super::labeled::{LabeledCounter, LabeledHistogram, format_labels};

/// Metrics registry for managing multiple metrics
#[derive(Default, Clone)]
pub struct MetricsRegistry {
    counters: Arc<RwLock<HashMap<String, Counter>>>,
    gauges: Arc<RwLock<HashMap<String, Gauge>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    labeled_counters: Arc<RwLock<HashMap<String, LabeledCounter>>>,
    labeled_histograms: Arc<RwLock<HashMap<String, LabeledHistogram>>>,
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

    /// Get or create a labeled counter
    pub fn labeled_counter(&self, name: &str) -> LabeledCounter {
        let lc = self
            .labeled_counters
            .read()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(c) = lc.get(name) {
            return c.clone();
        }
        drop(lc);

        let mut lc = self
            .labeled_counters
            .write()
            .unwrap_or_else(|e| e.into_inner());
        lc.entry(name.to_string()).or_default().clone()
    }

    /// Get or create a labeled histogram
    pub fn labeled_histogram(&self, name: &str) -> LabeledHistogram {
        let lh = self
            .labeled_histograms
            .read()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(h) = lh.get(name) {
            return h.clone();
        }
        drop(lh);

        let mut lh = self
            .labeled_histograms
            .write()
            .unwrap_or_else(|e| e.into_inner());
        lh.entry(name.to_string()).or_default().clone()
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
            for (bound, count) in histogram.bucket_counts() {
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

        // Export labeled counters
        let labeled_counters = self
            .labeled_counters
            .read()
            .unwrap_or_else(|e| e.into_inner());
        for (name, lc) in labeled_counters.iter() {
            output.push_str(&format!("# TYPE {} counter\n", name));
            for (labels, value) in lc.entries() {
                let label_str = format_labels(&labels);
                output.push_str(&format!("{}{} {}\n", name, label_str, value));
            }
        }

        // Export labeled histograms
        let labeled_histograms = self
            .labeled_histograms
            .read()
            .unwrap_or_else(|e| e.into_inner());
        for (name, lh) in labeled_histograms.iter() {
            output.push_str(&format!("# TYPE {} histogram\n", name));
            for (labels, histogram) in lh.entries() {
                let label_str = format_labels(&labels);
                for (bound, count) in histogram.bucket_counts() {
                    let mut all_labels = labels.clone();
                    all_labels.push(("le".to_string(), format!("{}", bound)));
                    let bucket_label_str = format_labels(&all_labels);
                    output.push_str(&format!("{}_bucket{} {}\n", name, bucket_label_str, count));
                }
                let mut inf_labels = labels.clone();
                inf_labels.push(("le".to_string(), "+Inf".to_string()));
                let inf_label_str = format_labels(&inf_labels);
                output.push_str(&format!(
                    "{}_bucket{} {}\n",
                    name,
                    inf_label_str,
                    histogram.count()
                ));
                output.push_str(&format!("{}_sum{} {}\n", name, label_str, histogram.sum()));
                output.push_str(&format!(
                    "{}_count{} {}\n",
                    name,
                    label_str,
                    histogram.count()
                ));
            }
        }

        output
    }
}
