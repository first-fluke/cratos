use super::*;
use reqwest::header::{HeaderMap, HeaderValue};

#[tokio::test]
async fn test_quota_tracker_update_and_get() {
    let tracker = QuotaTracker::new();

    let mut headers = HeaderMap::new();
    headers.insert(
        "anthropic-ratelimit-requests-limit",
        HeaderValue::from_static("1000"),
    );
    headers.insert(
        "anthropic-ratelimit-requests-remaining",
        HeaderValue::from_static("999"),
    );

    tracker.update_from_headers("anthropic", &headers).await;

    let state = tracker.get_state("anthropic").await.unwrap();
    assert_eq!(state.requests_limit, Some(1000));
    assert_eq!(state.requests_remaining, Some(999));
}

#[tokio::test]
async fn test_quota_tracker_incremental_merge() {
    let tracker = QuotaTracker::new();

    // First update: requests only
    let mut h1 = HeaderMap::new();
    h1.insert(
        "anthropic-ratelimit-requests-limit",
        HeaderValue::from_static("1000"),
    );
    h1.insert(
        "anthropic-ratelimit-requests-remaining",
        HeaderValue::from_static("999"),
    );
    tracker.update_from_headers("anthropic", &h1).await;

    // Second update: tokens only
    let mut h2 = HeaderMap::new();
    h2.insert(
        "anthropic-ratelimit-tokens-limit",
        HeaderValue::from_static("100000"),
    );
    h2.insert(
        "anthropic-ratelimit-tokens-remaining",
        HeaderValue::from_static("50000"),
    );
    tracker.update_from_headers("anthropic", &h2).await;

    let state = tracker.get_state("anthropic").await.unwrap();
    assert_eq!(state.requests_limit, Some(1000));
    assert_eq!(state.tokens_limit, Some(100_000));
}

#[tokio::test]
async fn test_quota_tracker_get_all() {
    let tracker = QuotaTracker::new();

    let mut h = HeaderMap::new();
    h.insert("x-ratelimit-limit-requests", HeaderValue::from_static("60"));
    h.insert(
        "x-ratelimit-remaining-requests",
        HeaderValue::from_static("30"),
    );

    tracker.update_from_headers("groq", &h).await;
    tracker.update_from_headers("deepseek", &h).await;

    let all = tracker.get_all_states().await;
    assert_eq!(all.len(), 2);
}

#[test]
fn test_quota_state_usage_pct() {
    let state = QuotaState {
        provider: "test".to_string(),
        requests_remaining: Some(100),
        requests_limit: Some(1000),
        tokens_remaining: Some(50_000),
        tokens_limit: Some(100_000),
        reset_at: None,
        updated_at: Utc::now(),
        source: QuotaSource::ResponseHeaders,
        remaining_fraction: None,
        tier_label: None,
    };
    assert!((state.requests_usage_pct().unwrap() - 90.0).abs() < 0.01);
    assert!((state.tokens_usage_pct().unwrap() - 50.0).abs() < 0.01);
}

#[test]
fn test_is_near_limit() {
    let state = QuotaState {
        provider: "test".to_string(),
        requests_remaining: Some(5),
        requests_limit: Some(100),
        tokens_remaining: Some(90_000),
        tokens_limit: Some(100_000),
        reset_at: None,
        updated_at: Utc::now(),
        source: QuotaSource::ResponseHeaders,
        remaining_fraction: None,
        tier_label: None,
    };
    // 95% usage, threshold 20% -> near limit
    assert!(state.is_near_limit(20.0));
}

#[test]
fn test_format_compact_number() {
    assert_eq!(format_compact_number(500), "500");
    assert_eq!(format_compact_number(1_500), "1.5K");
    assert_eq!(format_compact_number(45_200), "45.2K");
    assert_eq!(format_compact_number(1_500_000), "1.5M");
}

#[test]
fn test_format_duration() {
    assert_eq!(format_duration(&chrono::Duration::seconds(45)), "45s");
    assert_eq!(format_duration(&chrono::Duration::seconds(134)), "2m 14s");
    assert_eq!(format_duration(&chrono::Duration::seconds(3723)), "1h 02m");
    assert_eq!(format_duration(&chrono::Duration::seconds(0)), "now");
}

#[tokio::test]
async fn test_update_from_retry_after() {
    let tracker = QuotaTracker::new();
    tracker.update_from_retry_after("gemini", 30).await;

    let state = tracker.get_state("gemini").await.unwrap();
    assert_eq!(state.requests_remaining, Some(0));
    assert!(state.reset_at.is_some());
    assert_eq!(state.source, QuotaSource::RetryAfter);
}

#[test]
fn test_remaining_pct_from_fraction() {
    let state = QuotaState {
        provider: "gemini".to_string(),
        requests_remaining: None,
        requests_limit: None,
        tokens_remaining: None,
        tokens_limit: None,
        reset_at: None,
        updated_at: Utc::now(),
        source: QuotaSource::PolledApi,
        remaining_fraction: Some(0.85),
        tier_label: None,
    };
    let pct = state.remaining_pct().unwrap();
    assert!((pct - 85.0).abs() < 0.01);
}

#[test]
fn test_remaining_pct_from_requests() {
    let state = QuotaState {
        provider: "anthropic".to_string(),
        requests_remaining: Some(750),
        requests_limit: Some(1000),
        tokens_remaining: None,
        tokens_limit: None,
        reset_at: None,
        updated_at: Utc::now(),
        source: QuotaSource::ResponseHeaders,
        remaining_fraction: None,
        tier_label: None,
    };
    let pct = state.remaining_pct().unwrap();
    assert!((pct - 75.0).abs() < 0.01);
}

#[test]
fn test_remaining_pct_none() {
    let state = QuotaState {
        provider: "unknown".to_string(),
        requests_remaining: None,
        requests_limit: None,
        tokens_remaining: None,
        tokens_limit: None,
        reset_at: None,
        updated_at: Utc::now(),
        source: QuotaSource::ResponseHeaders,
        remaining_fraction: None,
        tier_label: None,
    };
    assert!(state.remaining_pct().is_none());
}

#[tokio::test]
async fn test_update_state_direct() {
    let tracker = QuotaTracker::new();
    let state = QuotaState {
        provider: "gemini".to_string(),
        requests_remaining: None,
        requests_limit: None,
        tokens_remaining: None,
        tokens_limit: None,
        reset_at: None,
        updated_at: Utc::now(),
        source: QuotaSource::PolledApi,
        remaining_fraction: Some(0.42),
        tier_label: Some("free".to_string()),
    };
    tracker.update_state(state).await;

    let retrieved = tracker.get_state("gemini").await.unwrap();
    assert_eq!(retrieved.source, QuotaSource::PolledApi);
    assert!((retrieved.remaining_fraction.unwrap() - 0.42).abs() < 0.001);
    assert_eq!(retrieved.tier_label.as_deref(), Some("free"));
}

#[test]
fn test_parse_empty_headers() {
    let headers = HeaderMap::new();
    let partial = super::providers::anthropic::parse_anthropic_headers(&headers);
    assert!(!partial.has_data());
}
