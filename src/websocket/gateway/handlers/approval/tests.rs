use super::super::super::dispatch::DispatchContext;
use crate::websocket::gateway::browser_relay::BrowserRelay;
use crate::websocket::protocol::{GatewayErrorCode, GatewayFrame};
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

fn readonly_auth() -> AuthContext {
    AuthContext {
        user_id: "reader".to_string(),
        method: AuthMethod::ApiKey,
        scopes: vec![Scope::SessionRead, Scope::ExecutionRead],
        session_id: None,
        device_id: None,
    }
}

fn test_browser_relay() -> crate::websocket::gateway::browser_relay::SharedBrowserRelay {
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
async fn test_approval_respond_scope() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let nr = NodeRegistry::new(pool);
    let a2a = A2aRouter::new(100);
    let br = test_browser_relay();
    let orch = test_orchestrator();
    let eb = test_event_bus();

    // Readonly user should be forbidden
    let ro = readonly_auth();
    let ctx = DispatchContext {
        auth: &ro,
        node_registry: &nr,
        a2a_router: &a2a,
        browser_relay: &br,
        orchestrator: &orch,
        event_bus: &eb,
        approval_manager: None,
    };
    let result = super::handle("9", "approval.respond", serde_json::json!({}), &ctx).await;
    match result {
        GatewayFrame::Response { error: Some(e), .. } => {
            assert_eq!(e.code, GatewayErrorCode::Forbidden);
        }
        _ => panic!("expected error"),
    }

    // Admin without approval_manager → returns ok with "not configured" message
    let admin = admin_auth();
    let ctx = DispatchContext {
        auth: &admin,
        node_registry: &nr,
        a2a_router: &a2a,
        browser_relay: &br,
        orchestrator: &orch,
        event_bus: &eb,
        approval_manager: None,
    };
    let result = super::handle("10", "approval.respond", serde_json::json!({"request_id": "00000000-0000-0000-0000-000000000000", "approved": true}), &ctx).await;
    assert!(matches!(
        result,
        GatewayFrame::Response {
            result: Some(_),
            ..
        }
    ));
}

#[tokio::test]
async fn test_approval_respond_invalid_request_id() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let nr = NodeRegistry::new(pool);
    let a2a = A2aRouter::new(100);
    let br = test_browser_relay();
    let orch = test_orchestrator();
    let eb = test_event_bus();
    let admin = admin_auth();
    let ctx = DispatchContext {
        auth: &admin,
        node_registry: &nr,
        a2a_router: &a2a,
        browser_relay: &br,
        orchestrator: &orch,
        event_bus: &eb,
        approval_manager: None,
    };
    let result = super::handle(
        "11",
        "approval.respond",
        serde_json::json!({"request_id": "not-a-uuid"}),
        &ctx,
    )
    .await;
    match result {
        GatewayFrame::Response { error: Some(e), .. } => {
            assert_eq!(e.code, GatewayErrorCode::InvalidParams);
        }
        _ => panic!("expected error"),
    }
}

#[tokio::test]
async fn test_approval_list_pending() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let nr = NodeRegistry::new(pool);
    let a2a = A2aRouter::new(100);
    let br = test_browser_relay();
    let orch = test_orchestrator();
    let eb = test_event_bus();
    let admin = admin_auth();
    let ctx = DispatchContext {
        auth: &admin,
        node_registry: &nr,
        a2a_router: &a2a,
        browser_relay: &br,
        orchestrator: &orch,
        event_bus: &eb,
        approval_manager: None,
    };
    let result = super::handle("12", "approval.list", serde_json::json!({}), &ctx).await;
    match result {
        GatewayFrame::Response {
            result: Some(v), ..
        } => {
            assert_eq!(v["count"], 0);
        }
        _ => panic!("expected ok"),
    }
}
