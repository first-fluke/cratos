use super::*;
use config::DEFAULT_BATCH_SIZE;
use config::DEFAULT_FLUSH_INTERVAL_SECS;
use std::sync::atomic::Ordering;

fn create_test_config(enabled: bool, batch_size: usize) -> TelemetryConfig {
    TelemetryConfig {
        enabled,
        batch_size,
        ..Default::default()
    }
}

#[test]
fn test_default_config() {
    let config = TelemetryConfig::default();

    assert!(!config.anonymous_id.is_empty());
    assert_eq!(config.batch_size, DEFAULT_BATCH_SIZE);
    assert_eq!(config.flush_interval_secs, DEFAULT_FLUSH_INTERVAL_SECS);
}

#[tokio::test]
async fn test_telemetry_disabled_does_not_queue() {
    let telemetry = Telemetry::new(create_test_config(false, DEFAULT_BATCH_SIZE));
    assert!(!telemetry.is_enabled());

    telemetry
        .track(TelemetryEvent::CommandExecuted {
            command: "test".to_string(),
            duration_ms: 100,
            success: true,
        })
        .await;
}

#[tokio::test]
async fn test_stats_update_even_when_disabled() {
    let telemetry = Telemetry::new(create_test_config(false, DEFAULT_BATCH_SIZE));

    telemetry
        .track(TelemetryEvent::CommandExecuted {
            command: "test".to_string(),
            duration_ms: 100,
            success: true,
        })
        .await;

    telemetry
        .track(TelemetryEvent::CommandExecuted {
            command: "test".to_string(),
            duration_ms: 100,
            success: false,
        })
        .await;

    assert_eq!(
        telemetry.stats().commands_executed.load(Ordering::Relaxed),
        2
    );
    assert_eq!(
        telemetry.stats().commands_succeeded.load(Ordering::Relaxed),
        1
    );
    assert!((telemetry.stats().success_rate() - 0.5).abs() < f64::EPSILON);
}
