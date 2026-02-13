//! Tests for cost module

use super::*;
use chrono::Utc;

#[test]
fn test_model_pricing_calculation() {
    let pricing = ModelPricing {
        model: "test-model".to_string(),
        provider: "test".to_string(),
        input_cost_per_million: 10.0,
        output_cost_per_million: 20.0,
        context_window: 128_000,
        updated_at: Utc::now(),
    };

    // 1M tokens each
    let cost = pricing.calculate_cost(1_000_000, 1_000_000);
    assert!((cost - 30.0).abs() < 0.001);

    // 1K tokens each
    let cost = pricing.calculate_cost(1_000, 1_000);
    assert!((cost - 0.03).abs() < 0.001);
}

#[test]
fn test_default_pricing_has_common_models() {
    let pricing = default_pricing();

    // OpenAI GPT-5
    assert!(pricing.contains_key("gpt-5"));
    assert!(pricing.contains_key("gpt-5-nano"));
    // OpenAI GPT-4o (legacy)
    assert!(pricing.contains_key("gpt-4o"));
    assert!(pricing.contains_key("gpt-4o-mini"));

    // Anthropic Claude 4.5
    assert!(pricing.contains_key("claude-opus-4-5-20250514"));
    assert!(pricing.contains_key("claude-sonnet-4-5-20250929"));
    assert!(pricing.contains_key("claude-haiku-4-5-20251001"));
    // Anthropic legacy
    assert!(pricing.contains_key("claude-sonnet-4-20250514"));

    // Gemini
    assert!(pricing.contains_key("gemini-2.5-flash"));
    assert!(pricing.contains_key("gemini-2.5-pro"));

    // Groq
    assert!(pricing.contains_key("llama-3.3-70b-versatile"));
    assert!(pricing.contains_key("openai/gpt-oss-20b"));

    // DeepSeek
    assert!(pricing.contains_key("deepseek-r1-distill-llama-70b"));

    // Free models
    assert!(pricing.contains_key("llama-3.3-70b-versatile"));
    assert!(pricing.contains_key("qwen2.5:7b"));
}

#[tokio::test]
async fn test_cost_tracker_record_and_stats() {
    let tracker = CostTracker::new();

    // Record some usage
    tracker
        .record_usage("openai", "gpt-4o-mini", 1000, 500, 100, true, None)
        .await;
    tracker
        .record_usage(
            "anthropic",
            "claude-3-5-sonnet-20241022",
            2000,
            1000,
            200,
            true,
            None,
        )
        .await;
    tracker
        .record_usage("openai", "gpt-4o", 500, 200, 150, false, None)
        .await;

    let stats = tracker.get_stats(None).await;

    assert_eq!(stats.total_requests, 3);
    assert_eq!(stats.successful_requests, 2);
    assert_eq!(stats.failed_requests, 1);
    assert_eq!(stats.total_input_tokens, 3500);
    assert_eq!(stats.total_output_tokens, 1700);
    assert!(stats.total_cost > 0.0);
}

#[tokio::test]
async fn test_estimate_cost() {
    let tracker = CostTracker::new();

    // GPT-5.2-mini: $0.15/1M input, $0.60/1M output
    let cost = tracker
        .estimate_cost("gpt-4o-mini", 1_000_000, 1_000_000)
        .await;
    assert!((cost - 0.75).abs() < 0.01);

    // Unknown model should use default
    let cost = tracker
        .estimate_cost("unknown-model", 1_000_000, 1_000_000)
        .await;
    assert!(cost > 0.0);
}

#[tokio::test]
async fn test_get_execution_records() {
    let tracker = CostTracker::new();

    tracker
        .record_usage(
            "openai",
            "gpt-4o",
            100,
            50,
            100,
            true,
            Some("exec-1".to_string()),
        )
        .await;
    tracker
        .record_usage(
            "openai",
            "gpt-4o",
            200,
            100,
            100,
            true,
            Some("exec-1".to_string()),
        )
        .await;
    tracker
        .record_usage(
            "openai",
            "gpt-4o",
            150,
            75,
            100,
            true,
            Some("exec-2".to_string()),
        )
        .await;

    let exec1_records = tracker.get_execution_records("exec-1").await;
    assert_eq!(exec1_records.len(), 2);

    let exec2_records = tracker.get_execution_records("exec-2").await;
    assert_eq!(exec2_records.len(), 1);
}

#[tokio::test]
async fn test_generate_report() {
    let tracker = CostTracker::new();

    tracker
        .record_usage("openai", "gpt-4o", 10000, 5000, 100, true, None)
        .await;
    tracker
        .record_usage(
            "anthropic",
            "claude-3-5-sonnet-20241022",
            20000,
            10000,
            200,
            true,
            None,
        )
        .await;

    let report = tracker.generate_report(None).await;

    assert_eq!(report.stats.total_requests, 2);
    assert!(report.stats.total_cost > 0.0);
    assert!(!report.stats.by_provider.is_empty());
    assert!(!report.stats.by_model.is_empty());
}

#[tokio::test]
async fn test_format_report() {
    let tracker = CostTracker::new();

    tracker
        .record_usage("openai", "gpt-4o", 10000, 5000, 100, true, None)
        .await;

    let report = tracker.generate_report(None).await;
    let formatted = CostTracker::format_report(&report);

    assert!(formatted.contains("Cost Report"));
    assert!(formatted.contains("Total Requests"));
    assert!(formatted.contains("openai"));
}

#[test]
fn test_global_tracker() {
    let tracker1 = global_tracker();
    let tracker2 = global_tracker();

    // Should be the same instance
    assert!(std::sync::Arc::ptr_eq(&tracker1, &tracker2));
}
