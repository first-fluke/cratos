use cratos_core::{Orchestrator, OrchestratorConfig, OrchestratorInput, ExecutionStatus};
use cratos_llm::{
    MockProvider,
    ToolCall, ToolCompletionResponse,
};
use cratos_tools::{Tool, ToolDefinition, ToolRegistry, ToolResult};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// A simple tool that sleeps for a specified duration.
#[derive(Debug)]
pub struct SleepTool {
    definition: ToolDefinition,
}

impl SleepTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition::new(
                "sleep",
                "Sleeps for a specified number of milliseconds"
            ).with_parameters(json!({
                "type": "object",
                "properties": {
                    "duration_ms": {
                        "type": "integer",
                        "description": "Duration to sleep in milliseconds"
                    }
                },
                "required": ["duration_ms"]
            })),
        }
    }
}

#[async_trait::async_trait]
impl Tool for SleepTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, cratos_tools::Error> {
        let duration_ms = args["duration_ms"].as_u64().unwrap_or(100);
        sleep(Duration::from_millis(duration_ms)).await;
        Ok(ToolResult::success(json!(format!("Slept for {} ms", duration_ms)), duration_ms))
    }
}

#[tokio::test]
async fn test_steering_abort() {
    // 1. Setup MockProvider
    // capable of function calling
    let provider = Arc::new(MockProvider::default());
    
    // 2. Setup ToolRegistry
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(SleepTool::new()));
    let registry = Arc::new(registry);

    // 3. Setup Orchestrator
    let config = OrchestratorConfig::default();
    let orchestrator = Orchestrator::new(provider.clone(), registry, config);

    // 4. Prepare Mock Responses
    // Step 1: LLM calls "sleep" tool
    provider.add_tool_response(ToolCompletionResponse {
        content: Some("I will sleep for 2 seconds.".to_string()),
        tool_calls: vec![ToolCall {
            id: "call_1".to_string(),
            name: "sleep".to_string(),
            arguments: json!({"duration_ms": 2000}).to_string(),
            thought_signature: None,
        }],
        usage: None,
        finish_reason: Some("tool_calls".to_string()),
        model: "mock-model".to_string(),
    });
    
    // Step 2: (After tool execution, usually LLM responds again)
    // But we expect ABORT to happen during tool execution or right after
    
    let orchestrator = Arc::new(orchestrator);
    let orch_clone = orchestrator.clone();

    let input = OrchestratorInput::new("test", "test-session", "user", "Sleep for a while");

    // Spawn execution
    let handle = tokio::spawn(async move {
        orch_clone.process(input).await
    });

    // Wait for execution to start and get handle
    sleep(Duration::from_millis(500)).await;
    
    // We need to find the execution ID to steer it.
    // The test wrapper doesn't have easy access to the ID unless we intercept events or list active executions.
    // Orchestrator exposes active_executions map.
    
    // Retry getting execution ID
    let mut exec_id = None;
    for _ in 0..10 {
        if let Some(entry) = orchestrator.active_executions().iter().next() {
            exec_id = Some(*entry.key());
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    
    let exec_id = exec_id.expect("Execution should have started");
    
    // 5. Send Abort Signal
    let steer_handle = orchestrator.get_steer_handle(exec_id).expect("Steer handle should exist");
    steer_handle.abort(Some("Test abort".to_string())).await.expect("Failed to abort");

    // 6. Await result
    let result = handle.await.expect("Task panicked").expect("Orchestrator failed");
    
    // 7. Verify Cancellation
    assert_eq!(result.status, ExecutionStatus::Cancelled);
    assert!(result.response.contains("Test abort"));
}
