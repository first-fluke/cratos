//! Integration tests for Cratos
//!
//! These tests verify the integration between different crates:
//! - cratos-replay: Event storage and retrieval
//! - cratos-llm: LLM provider configuration
//! - cratos-tools: Tool registry and execution
//! - cratos-core: Orchestrator and session management
//! - cratos-channels: Message normalization

use std::sync::Arc;
use std::time::Duration;

// Re-export crates for testing
use cratos_core::{OrchestratorConfig, OrchestratorInput};
use cratos_llm::{
    CompletionRequest, Message, MessageRole, ModelTier, RouterConfig, RoutingRules, TaskType,
};
use cratos_replay::{Event, EventType, Execution, ExecutionStatus};
use cratos_tools::{
    builtins::register_builtins, ExecutionOptions, RunnerConfig, ToolRegistry, ToolRunner,
};

// ============================================================================
// LLM Router Integration Tests
// ============================================================================

#[test]
fn test_llm_router_configuration() {
    let config = RouterConfig::default();

    assert_eq!(config.default_provider, "openai");
    assert!(config.providers.is_empty());
}

#[test]
fn test_routing_rules_default() {
    let rules = RoutingRules::default();

    // Default rules should have sensible defaults
    assert!(!rules.prefer_local);
    assert!(rules.max_tier.is_none());
}

#[test]
fn test_model_tier_configuration() {
    // Fast tier should use cheaper models (2026: gpt-5-nano)
    let fast_model = ModelTier::Fast.default_model("openai");
    assert!(fast_model.contains("nano") || fast_model.contains("mini"));

    // Premium tier should use advanced models (2026: claude-opus-4.5)
    let premium_model = ModelTier::Premium.default_model("anthropic");
    assert!(premium_model.contains("opus"));
}

#[test]
fn test_task_types_coverage() {
    // Verify all task types can be created
    let task_types = vec![
        TaskType::Classification,
        TaskType::Planning,
        TaskType::CodeGeneration,
        TaskType::Summarization,
        TaskType::Conversation,
        TaskType::Extraction,
        TaskType::CodeReview,
        TaskType::Translation,
    ];

    for task_type in task_types {
        let tier = ModelTier::Fast;
        let model = tier.default_model("openai");
        assert!(
            !model.is_empty(),
            "Task {:?} should map to a model",
            task_type
        );
    }
}

#[test]
fn test_model_tier_all_providers() {
    // Test that each tier has default models for each provider
    let providers = ["openai", "anthropic", "gemini", "ollama"];
    let tiers = [ModelTier::Fast, ModelTier::Standard, ModelTier::Premium];

    for tier in tiers {
        for provider in providers {
            let model = tier.default_model(provider);
            assert!(
                !model.is_empty(),
                "Tier {:?} should have model for {}",
                tier,
                provider
            );
        }
    }
}

// ============================================================================
// Tool Registry Integration Tests
// ============================================================================

#[test]
fn test_tool_registry_with_builtins() {
    let mut registry = ToolRegistry::new();
    register_builtins(&mut registry);

    // Verify all expected tools are registered
    let expected_tools = [
        "file_read",
        "file_write",
        "file_list",
        "http_get",
        "http_post",
        "exec",
        "bash",
        "git_status",
        "git_commit",
        "git_branch",
        "git_diff",
        "git_push",
        "git_clone",
        "git_log",
        "github_api",
        "browser",
        "wol",
        "config",
        "web_search",
        "agent_cli",
        "send_file",
    ];

    for tool_name in expected_tools {
        assert!(
            registry.has(tool_name),
            "Tool '{}' should be registered",
            tool_name
        );
    }

    assert_eq!(registry.len(), expected_tools.len());
}

#[test]
fn test_tool_definitions_have_schemas() {
    let mut registry = ToolRegistry::new();
    register_builtins(&mut registry);

    for def in registry.list_definitions() {
        assert!(!def.name.is_empty(), "Tool name should not be empty");
        assert!(
            !def.description.is_empty(),
            "Tool '{}' should have description",
            def.name
        );
        assert!(
            !def.parameters.is_null(),
            "Tool '{}' should have parameters",
            def.name
        );
    }
}

#[test]
fn test_tool_runner_configuration() {
    let registry = Arc::new(ToolRegistry::new());
    let config = RunnerConfig::new(Duration::from_secs(30))
        .with_max_timeout(Duration::from_secs(120))
        .with_sandbox(true)
        .with_high_risk(false);

    let runner = ToolRunner::new(Arc::clone(&registry), config);

    assert_eq!(runner.config().default_timeout, Duration::from_secs(30));
    assert_eq!(runner.config().max_timeout, Duration::from_secs(120));
    assert!(runner.config().sandbox_enabled);
    assert!(!runner.config().allow_high_risk);
}

// ============================================================================
// Replay System Integration Tests
// ============================================================================

#[test]
fn test_execution_lifecycle() {
    let execution = Execution::new("telegram", "C123", "U456", "Hello, Cratos!");

    assert!(!execution.id.is_nil());
    assert_eq!(execution.channel_type, "telegram");
    assert_eq!(execution.channel_id, "C123");
    assert_eq!(execution.user_id, "U456");
    assert_eq!(execution.input_text, "Hello, Cratos!");
    assert_eq!(execution.status, ExecutionStatus::Pending);
}

#[test]
fn test_execution_with_thread() {
    let mut execution = Execution::new("slack", "C123", "U456", "Hello!");
    execution.thread_id = Some("T789".to_string());

    assert_eq!(execution.thread_id, Some("T789".to_string()));
}

#[test]
fn test_event_creation() {
    let execution = Execution::new("telegram", "C123", "U456", "Test input");

    let user_input_event = Event::new(execution.id, 1, EventType::UserInput);

    assert_eq!(user_input_event.execution_id, execution.id);
    assert_eq!(user_input_event.sequence_num, 1);
    assert!(matches!(user_input_event.event_type, EventType::UserInput));

    let llm_request_event = Event::new(execution.id, 2, EventType::LlmRequest);

    assert_eq!(llm_request_event.sequence_num, 2);
    assert!(matches!(
        llm_request_event.event_type,
        EventType::LlmRequest
    ));
}

#[test]
fn test_event_types_coverage() {
    // Verify all event types can be created
    let event_types = vec![
        EventType::UserInput,
        EventType::PlanCreated,
        EventType::LlmRequest,
        EventType::LlmResponse,
        EventType::ToolCall,
        EventType::ToolResult,
        EventType::FinalResponse,
        EventType::Error,
    ];

    let execution = Execution::new("test", "C1", "U1", "Input");
    for (i, event_type) in event_types.into_iter().enumerate() {
        let event = Event::new(execution.id, i as i32 + 1, event_type);
        assert!(event.sequence_num > 0);
    }
}

// ============================================================================
// Core Orchestrator Integration Tests
// ============================================================================

#[test]
fn test_orchestrator_input_creation() {
    let input = OrchestratorInput::new("telegram", "C123", "U456", "Help me with a task");

    assert_eq!(input.channel_type, "telegram");
    assert_eq!(input.channel_id, "C123");
    assert_eq!(input.user_id, "U456");
    assert_eq!(input.text, "Help me with a task");
    assert!(input.thread_id.is_none());
}

#[test]
fn test_orchestrator_input_with_thread() {
    let input = OrchestratorInput::new("slack", "C123", "U456", "Continue the conversation")
        .with_thread("T789".to_string());

    assert_eq!(input.thread_id, Some("T789".to_string()));
}

#[test]
fn test_orchestrator_config() {
    let config = OrchestratorConfig::default();

    // Default config should have reasonable defaults
    assert!(config.max_iterations > 0);
}

#[test]
fn test_orchestrator_input_session_key() {
    let input = OrchestratorInput::new("telegram", "C123", "U456", "Hello");

    let key = input.session_key();
    assert!(key.contains("telegram"));
    assert!(key.contains("C123"));
    assert!(key.contains("U456"));
}

// ============================================================================
// Message Flow Integration Tests
// ============================================================================

#[test]
fn test_message_role_conversion() {
    let roles = vec![
        MessageRole::System,
        MessageRole::User,
        MessageRole::Assistant,
    ];

    for role in roles {
        let message = Message {
            role,
            content: "Test message".to_string(),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
            images: Vec::new(),
        };
        assert!(!message.content.is_empty());
    }
}

#[test]
fn test_completion_request_builder() {
    let messages = vec![Message {
        role: MessageRole::System,
        content: "You are a helpful assistant.".to_string(),
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
        images: Vec::new(),
    }];

    let request = CompletionRequest::new("gpt-4o")
        .with_messages(messages)
        .with_max_tokens(1000)
        .with_temperature(0.7);

    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.model, "gpt-4o");
    assert_eq!(request.max_tokens, Some(1000));
    assert_eq!(request.temperature, Some(0.7));
}

// ============================================================================
// Cross-Crate Integration Tests
// ============================================================================

#[test]
fn test_full_pipeline_types() {
    // This test verifies that types from different crates can work together

    // 1. Create execution
    let execution = Execution::new("telegram", "C123", "U456", "Run a file operation");

    // 2. Create event for the execution
    let event = Event::new(execution.id, 1, EventType::UserInput);

    // 3. Create tool registry
    let mut registry = ToolRegistry::new();
    register_builtins(&mut registry);

    // 4. Create orchestrator input
    let input = OrchestratorInput::new(
        &execution.channel_type,
        &execution.channel_id,
        &execution.user_id,
        &execution.input_text,
    );

    // 5. Verify pipeline coherence
    assert_eq!(event.execution_id, execution.id);
    assert_eq!(input.channel_type, execution.channel_type);
    assert!(registry.has("file_read"));
}

#[tokio::test]
async fn test_tool_execution_with_runner() {
    let mut registry = ToolRegistry::new();
    register_builtins(&mut registry);

    let runner = ToolRunner::with_defaults(Arc::new(registry));

    // Test dry run execution
    let options = ExecutionOptions::dry_run();
    let result = runner
        .execute_with_options(
            "file_read",
            serde_json::json!({"path": "/tmp/test.txt"}),
            options,
        )
        .await;

    assert!(result.is_ok());
    let exec_result = result.unwrap();
    assert!(exec_result.dry_run);
}

#[test]
fn test_session_key_format() {
    // Session keys should be deterministic for the same input
    let input1 = OrchestratorInput::new("telegram", "C123", "U456", "Hello");
    let input2 = OrchestratorInput::new("telegram", "C123", "U456", "World");

    assert_eq!(input1.session_key(), input2.session_key());

    // Different channel should produce different keys
    let input3 = OrchestratorInput::new("slack", "C123", "U456", "Hello");
    assert_ne!(input1.session_key(), input3.session_key());
}

// ============================================================================
// Error Handling Integration Tests
// ============================================================================

#[test]
fn test_execution_status_transitions() {
    let mut execution = Execution::new("telegram", "C123", "U456", "Test");

    assert_eq!(execution.status, ExecutionStatus::Pending);

    execution.status = ExecutionStatus::Running;
    assert_eq!(execution.status, ExecutionStatus::Running);

    execution.status = ExecutionStatus::Completed;
    assert_eq!(execution.status, ExecutionStatus::Completed);
}

#[test]
fn test_error_event_creation() {
    let execution = Execution::new("test", "C1", "U1", "Input");

    let mut error_event = Event::new(execution.id, 1, EventType::Error);
    error_event.payload = serde_json::json!({
        "error": "Something went wrong",
        "code": "INTERNAL_ERROR"
    });

    assert!(matches!(error_event.event_type, EventType::Error));
    assert!(error_event.payload.get("error").is_some());
}

// ============================================================================
// Channel Message Integration Tests
// ============================================================================

#[test]
fn test_normalized_message_from_telegram() {
    use cratos_channels::{ChannelType, NormalizedMessage};

    let msg = NormalizedMessage::new(
        ChannelType::Telegram,
        "12345".to_string(),
        "user123".to_string(),
        "msg456".to_string(),
        "Hello, bot!".to_string(),
    );

    assert!(matches!(msg.channel_type, ChannelType::Telegram));
    assert_eq!(msg.channel_id, "12345");
    assert_eq!(msg.user_id, "user123");
    assert_eq!(msg.text, "Hello, bot!");
}

#[test]
fn test_normalized_message_with_thread() {
    use cratos_channels::{ChannelType, NormalizedMessage};

    let msg = NormalizedMessage::new(
        ChannelType::Slack,
        "C123".to_string(),
        "U456".to_string(),
        "ts123".to_string(),
        "Reply message".to_string(),
    )
    .with_thread("thread_ts".to_string());

    assert_eq!(msg.thread_id, Some("thread_ts".to_string()));
}

#[test]
fn test_outgoing_message_builder() {
    use cratos_channels::OutgoingMessage;

    let msg = OutgoingMessage::text("Hello!").in_thread("thread123".to_string());

    assert_eq!(msg.text, "Hello!");
    assert_eq!(msg.thread_id, Some("thread123".to_string()));
}

// ============================================================================
// Rate Limiter Integration Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limiter_integration() {
    use cratos_core::{RateLimitConfig, RateLimiter};

    let config = RateLimitConfig::per_minute(10);
    let limiter = RateLimiter::new(config);

    // Simulate multiple requests from same user
    for i in 0..10 {
        let result = limiter.acquire("user:123").await;
        assert!(result.allowed, "Request {} should be allowed", i);
    }

    // 11th request should be denied
    let result = limiter.acquire("user:123").await;
    assert!(!result.allowed, "Request should be denied after limit");
    assert_eq!(result.remaining, 0);

    // Different user should still be allowed
    let result = limiter.acquire("user:456").await;
    assert!(result.allowed, "Different user should be allowed");
}

#[tokio::test]
async fn test_tiered_rate_limiter_integration() {
    use cratos_core::{RateLimitConfig, TieredRateLimiter};

    let tiered = TieredRateLimiter::new(
        RateLimitConfig::per_minute(5),  // Per user: 5/min
        RateLimitConfig::per_minute(20), // Global: 20/min
    );

    // User 1 uses their limit
    for _ in 0..5 {
        let result = tiered.acquire("user:1").await;
        assert!(result.allowed);
    }

    // User 1 is now limited
    let result = tiered.acquire("user:1").await;
    assert!(!result.allowed);

    // User 2 can still make requests
    let result = tiered.acquire("user:2").await;
    assert!(result.allowed);

    // Check usage
    let (user_current, user_max) = tiered.user_usage("user:1").await;
    assert_eq!(user_current, 5);
    assert_eq!(user_max, 5);

    let (global_current, global_max) = tiered.global_usage().await;
    // 5 from user1 + 1 from user1 denied (global recorded before user check) + 1 from user2 = 7
    assert_eq!(global_current, 7);
    assert_eq!(global_max, 20);
}

// ============================================================================
// Utility Integration Tests
// ============================================================================

#[tokio::test]
async fn test_circuit_breaker_integration() {
    use cratos_core::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
    use std::time::Duration;

    let config = CircuitBreakerConfig::new()
        .with_failure_threshold(3)
        .with_success_threshold(2)
        .with_reset_timeout(Duration::from_millis(100));

    let breaker = CircuitBreaker::new("test_breaker", config);

    // Should start closed
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());

    // Record failures to open circuit
    for _ in 0..3 {
        breaker.record_failure();
    }

    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());

    // Wait for reset timeout
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Should transition to half-open when we try to execute
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Record successes to close
    breaker.record_success();
    breaker.record_success();

    assert_eq!(breaker.state(), CircuitState::Closed);
}

#[test]
fn test_metrics_integration() {
    use cratos_core::MetricsRegistry;

    let registry = MetricsRegistry::new();

    // Test counter
    let counter = registry.counter("test_requests_total");
    counter.inc();
    counter.inc_by(5);
    assert_eq!(counter.get(), 6);

    // Test gauge
    let gauge = registry.gauge("test_active_connections");
    gauge.set(10);
    gauge.inc();
    gauge.dec();
    assert_eq!(gauge.get(), 10);

    // Test histogram
    let histogram = registry.histogram("test_request_duration");
    histogram.observe(10.0);
    histogram.observe(20.0);
    histogram.observe(30.0);
    assert_eq!(histogram.count(), 3);

    // Export to Prometheus format
    let output = registry.export_prometheus();
    assert!(output.contains("test_requests_total"));
    assert!(output.contains("test_active_connections"));
    assert!(output.contains("test_request_duration"));
}
