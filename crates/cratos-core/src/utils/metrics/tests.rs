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

    let buckets = histogram.bucket_counts();
    assert_eq!(buckets[0], (10.0, 1)); // 5 <= 10
    assert_eq!(buckets[1], (50.0, 2)); // 5, 25 <= 50
    assert_eq!(buckets[2], (100.0, 3)); // 5, 25, 75 <= 100
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
fn test_global_metrics() {
    global::counter("global_test").inc();
    assert_eq!(global::counter("global_test").get(), 1);
}

#[test]
fn test_labeled_counter() {
    let lc = LabeledCounter::new();
    lc.inc(&[("tool_name", "exec"), ("status", "ok")]);
    lc.inc(&[("tool_name", "exec"), ("status", "ok")]);
    lc.inc(&[("tool_name", "exec"), ("status", "error")]);
    lc.inc(&[("tool_name", "bash"), ("status", "ok")]);

    let entries = lc.entries();
    assert_eq!(entries.len(), 3);

    let exec_ok = entries
        .iter()
        .find(|(labels, _)| {
            labels.contains(&("tool_name".to_string(), "exec".to_string()))
                && labels.contains(&("status".to_string(), "ok".to_string()))
        })
        .unwrap();
    assert_eq!(exec_ok.1, 2);
}
