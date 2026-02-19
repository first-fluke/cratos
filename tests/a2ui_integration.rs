use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;

use cratos_canvas::a2ui::A2uiSessionManager;
use cratos_canvas::document::CanvasDocument;
use cratos_canvas::{CanvasSessionManager, CanvasState};
use cratos_tools::{
    register_builtins_with_config, BuiltinsConfig, ExecutionOptions, ToolRegistry, ToolRunner,
};

#[tokio::test]
async fn test_a2ui_tools_integration() {
    // 1. Setup Canvas State
    let session_manager = Arc::new(CanvasSessionManager::new());
    let (a2ui_tx, _a2ui_rx) = mpsc::channel(100);

    let canvas_state = Arc::new(CanvasState::new(session_manager.clone()).with_a2ui_tx(a2ui_tx));

    // 2. Setup A2UI Manager
    let a2ui_manager = Arc::new(A2uiSessionManager::new(canvas_state.clone()));

    // 3. Setup Tool Registry with A2UI enabled
    let mut registry = ToolRegistry::new();
    let config = BuiltinsConfig {
        a2ui_manager: Some(a2ui_manager),
        ..BuiltinsConfig::default()
    };

    register_builtins_with_config(&mut registry, &config);

    // 4. Verify Registration
    assert!(
        registry.has("a2ui_render"),
        "a2ui_render tool should be registered"
    );
    assert!(
        registry.has("a2ui_wait_event"),
        "a2ui_wait_event tool should be registered"
    );

    // 5. Create a Session to test execution
    let user_id = "test_user";
    let document = CanvasDocument::new("Test Document");
    let session = session_manager.create_session(user_id, document).await;
    let session_id = session.id;

    // Simulate a connected client by subscribing to broadcast channel
    // This prevents "no active connections" error
    let _rx = canvas_state.broadcast_tx.subscribe();

    // 6. Execute a2ui_render
    let runner = ToolRunner::with_defaults(Arc::new(registry));

    let args = json!({
        "session_id": session_id.to_string(),
        "component_type": "button",
        "props": {
            "label": "Click Me",
            "variant": "primary"
        },
        "slot": "main"
    });

    let result = runner
        .execute_with_options("a2ui_render", args, ExecutionOptions::default())
        .await;

    assert!(result.is_ok(), "Tool execution failed: {:?}", result.err());
    let exec_result = result.unwrap();
    // Access nested ToolResult fields
    assert!(
        exec_result.result.success,
        "Tool returned failure: {:?}",
        exec_result.result.error
    );

    let output = exec_result.result.output;
    assert!(
        output.get("component_id").is_some(),
        "Output should contain component_id"
    );
    assert_eq!(output.get("rendered"), Some(&json!(true)));
}
