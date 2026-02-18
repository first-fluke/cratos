use cratos_core::auth::Scope;

use super::super::dispatch::DispatchContext;
use crate::websocket::protocol::{GatewayError, GatewayErrorCode, GatewayFrame};

pub(crate) async fn handle(
    id: &str,
    method: &str,
    params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    match method {
        "a2a.send" => send(id, params, ctx).await,
        "a2a.list" => list(id, params, ctx).await,
        "a2a.history" => history(id, params, ctx).await,
        _ => GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::UnknownMethod,
                format!("Unknown method: {}", method),
            ),
        ),
    }
}

async fn send(id: &str, params: serde_json::Value, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ExecutionWrite) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires ExecutionWrite scope"),
        );
    }
    let from_agent = params
        .get("from_agent")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let to_agent = params
        .get("to_agent")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");

    if from_agent.is_empty() || to_agent.is_empty() || content.is_empty() {
        return GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::InvalidParams,
                "Missing required fields: from_agent, to_agent, content",
            ),
        );
    }

    let msg = cratos_core::a2a::A2aMessage::new(from_agent, to_agent, session_id, content);
    let msg_id = msg.id;
    ctx.a2a_router.send(msg).await;

    GatewayFrame::ok(
        id,
        serde_json::json!({
            "message_id": msg_id,
            "status": "delivered"
        }),
    )
}

async fn list(id: &str, params: serde_json::Value, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ExecutionRead) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires ExecutionRead scope"),
        );
    }
    let agent_id = params
        .get("agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if agent_id.is_empty() {
        return GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::InvalidParams,
                "Missing 'agent_id' parameter",
            ),
        );
    }

    let messages = ctx.a2a_router.peek(agent_id).await;
    let summaries: Vec<cratos_core::a2a::A2aMessageSummary> = messages
        .iter()
        .map(cratos_core::a2a::A2aMessageSummary::from)
        .collect();

    GatewayFrame::ok(
        id,
        serde_json::json!({
            "agent_id": agent_id,
            "count": summaries.len(),
            "messages": summaries,
        }),
    )
}

async fn history(id: &str, params: serde_json::Value, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ExecutionRead) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires ExecutionRead scope"),
        );
    }
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if session_id.is_empty() {
        return GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::InvalidParams,
                "Missing 'session_id' parameter",
            ),
        );
    }

    let summaries = ctx.a2a_router.session_history_summaries(session_id).await;

    GatewayFrame::ok(
        id,
        serde_json::json!({
            "session_id": session_id,
            "count": summaries.len(),
            "messages": summaries,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::super::super::dispatch::{dispatch_method, DispatchContext};
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
    async fn test_a2a_send() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let nr = NodeRegistry::new(pool);
        let a2a = A2aRouter::new(100);
        let auth = admin_auth();
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();
        let ctx = DispatchContext {
            auth: &auth,
            node_registry: &nr,
            a2a_router: &a2a,
            browser_relay: &br,
            orchestrator: &orch,
            event_bus: &eb,
            approval_manager: None,
        };

        let result = dispatch_method(
            "50",
            "a2a.send",
            serde_json::json!({
                "from_agent": "backend",
                "to_agent": "frontend",
                "session_id": "s1",
                "content": "API implementation complete"
            }),
            &ctx,
        )
        .await;
        match result {
            GatewayFrame::Response {
                result: Some(v), ..
            } => {
                assert_eq!(v["status"], "delivered");
                assert!(v.get("message_id").is_some());
            }
            _ => panic!("expected ok response"),
        }

        // Verify message was actually routed
        let msgs = a2a.peek("frontend").await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].from_agent, "backend");
    }

    #[tokio::test]
    async fn test_a2a_send_missing_fields() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let nr = NodeRegistry::new(pool);
        let a2a = A2aRouter::new(100);
        let auth = admin_auth();
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();
        let ctx = DispatchContext {
            auth: &auth,
            node_registry: &nr,
            a2a_router: &a2a,
            browser_relay: &br,
            orchestrator: &orch,
            event_bus: &eb,
            approval_manager: None,
        };

        let result = dispatch_method(
            "51",
            "a2a.send",
            serde_json::json!({"from_agent": "backend"}),
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
    async fn test_a2a_list() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let nr = NodeRegistry::new(pool);
        let a2a = A2aRouter::new(100);
        let auth = admin_auth();
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();
        let ctx = DispatchContext {
            auth: &auth,
            node_registry: &nr,
            a2a_router: &a2a,
            browser_relay: &br,
            orchestrator: &orch,
            event_bus: &eb,
            approval_manager: None,
        };

        // Send a message first
        a2a.send(cratos_core::a2a::A2aMessage::new(
            "backend", "frontend", "s1", "hello",
        ))
        .await;

        let result = dispatch_method(
            "52",
            "a2a.list",
            serde_json::json!({"agent_id": "frontend"}),
            &ctx,
        )
        .await;
        match result {
            GatewayFrame::Response {
                result: Some(v), ..
            } => {
                assert_eq!(v["count"], 1);
                assert_eq!(v["agent_id"], "frontend");
            }
            _ => panic!("expected ok response"),
        }
    }

    #[tokio::test]
    async fn test_a2a_history() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let nr = NodeRegistry::new(pool);
        let a2a = A2aRouter::new(100);
        let auth = admin_auth();
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();
        let ctx = DispatchContext {
            auth: &auth,
            node_registry: &nr,
            a2a_router: &a2a,
            browser_relay: &br,
            orchestrator: &orch,
            event_bus: &eb,
            approval_manager: None,
        };

        // Send some messages
        a2a.send(cratos_core::a2a::A2aMessage::new(
            "backend", "frontend", "s1", "msg1",
        ))
        .await;
        a2a.send(cratos_core::a2a::A2aMessage::new(
            "frontend", "qa", "s1", "msg2",
        ))
        .await;

        let result = dispatch_method(
            "53",
            "a2a.history",
            serde_json::json!({"session_id": "s1"}),
            &ctx,
        )
        .await;
        match result {
            GatewayFrame::Response {
                result: Some(v), ..
            } => {
                assert_eq!(v["count"], 2);
                assert_eq!(v["session_id"], "s1");
            }
            _ => panic!("expected ok response"),
        }
    }

    #[tokio::test]
    async fn test_a2a_send_without_scope() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let nr = NodeRegistry::new(pool);
        let a2a = A2aRouter::new(100);
        let auth = readonly_auth();
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();
        let ctx = DispatchContext {
            auth: &auth,
            node_registry: &nr,
            a2a_router: &a2a,
            browser_relay: &br,
            orchestrator: &orch,
            event_bus: &eb,
            approval_manager: None,
        };
        let result = dispatch_method(
            "54",
            "a2a.send",
            serde_json::json!({
                "from_agent": "a",
                "to_agent": "b",
                "content": "x"
            }),
            &ctx,
        )
        .await;
        match result {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::Forbidden);
            }
            _ => panic!("expected forbidden"),
        }
    }
}
