use super::super::protocol::{GatewayErrorCode, GatewayFrame};
use super::connection::handle_message;
use super::BrowserRelay;
use super::SharedBrowserRelay;
use cratos_core::a2a::A2aRouter;
use cratos_core::auth::{AuthContext, AuthMethod, Scope};
use cratos_core::event_bus::EventBus;
use cratos_core::nodes::NodeRegistry;
use cratos_core::{Orchestrator, OrchestratorConfig};
use cratos_tools::ToolRegistry;
use std::sync::Arc;

fn admin_auth() -> AuthContext {
    AuthContext {
        user_id: "admin".to_string(),
        method: AuthMethod::ApiKey,
        scopes: vec![
            Scope::Admin,
            Scope::SessionRead,
            Scope::SessionWrite,
            Scope::ExecutionRead,
            Scope::ExecutionWrite,
            Scope::ApprovalRespond,
            Scope::ConfigRead,
            Scope::ConfigWrite,
            Scope::NodeManage,
        ],
        session_id: None,
        device_id: None,
    }
}

fn test_a2a_router() -> A2aRouter {
    A2aRouter::new(100)
}

fn test_browser_relay() -> SharedBrowserRelay {
    Arc::new(BrowserRelay::new())
}

fn test_orchestrator() -> Arc<Orchestrator> {
    let provider: Arc<dyn cratos_llm::LlmProvider> = Arc::new(cratos_llm::MockProvider::new());
    let registry = Arc::new(ToolRegistry::new());
    Arc::new(Orchestrator::new(
        provider,
        registry,
        OrchestratorConfig::default(),
    ))
}

fn test_event_bus() -> Arc<EventBus> {
    Arc::new(EventBus::new(16))
}

#[tokio::test]
async fn test_gateway_dispatch() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let nr = NodeRegistry::new(pool);
    let a2a = test_a2a_router();
    let br = test_browser_relay();
    let orch = test_orchestrator();
    let eb = test_event_bus();

    // Just verify basic dispatch call (mocking a request)
    let req = GatewayFrame::Request {
        id: "1".to_string(),
        method: "unknown".to_string(), // Should return MethodNotFound
        params: serde_json::json!({}),
    };
    let json = serde_json::to_string(&req).unwrap();
    if let Some(GatewayFrame::Response { error: Some(e), .. }) =
        handle_message(&json, &admin_auth(), &nr, &a2a, &br, &orch, &eb, None).await
    {
        assert_eq!(e.code, GatewayErrorCode::UnknownMethod);
    } else {
        panic!("Expected error response");
    }
}

#[tokio::test]
async fn test_handle_message_invalid_json() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let nr = NodeRegistry::new(pool);

    let a2a = test_a2a_router();
    let br = test_browser_relay();
    let orch = test_orchestrator();
    let eb = test_event_bus();

    if let Some(GatewayFrame::Response { error: Some(e), .. }) =
        handle_message("not json", &admin_auth(), &nr, &a2a, &br, &orch, &eb, None).await
    {
        assert_eq!(e.code, GatewayErrorCode::InvalidParams);
    } else {
        panic!("expected error");
    }
}

#[tokio::test]
async fn test_handle_message_ignores_non_request() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let nr = NodeRegistry::new(pool);

    let a2a = test_a2a_router();
    let br = test_browser_relay();
    let orch = test_orchestrator();
    let eb = test_event_bus();
    // Response frame from client should be ignored
    let frame = GatewayFrame::Response {
        id: "1".to_string(),
        result: Some(serde_json::json!({})),
        error: None,
    };
    let json = serde_json::to_string(&frame).unwrap();
    let result = handle_message(&json, &admin_auth(), &nr, &a2a, &br, &orch, &eb, None).await;
    assert!(result.is_none());
}
