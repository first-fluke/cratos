use cratos_core::auth::Scope;
use cratos_core::OrchestratorInput;
use uuid::Uuid;

use crate::websocket::protocol::{GatewayError, GatewayErrorCode, GatewayFrame};
use super::super::dispatch::DispatchContext;

pub(crate) async fn handle(
    id: &str,
    method: &str,
    params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    match method {
        "chat.send" => send(id, params, ctx).await,
        "chat.cancel" => cancel(id, params, ctx).await,
        _ => GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::UnknownMethod,
                format!("Unknown method: {}", method),
            ),
        ),
    }
}

async fn send(
    id: &str,
    params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ExecutionWrite) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires ExecutionWrite scope"),
        );
    }
    let text = params
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if text.is_empty() {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::InvalidParams, "Missing 'text' parameter"),
        );
    }

    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    // Build orchestrator input
    let input = OrchestratorInput::new(
        "websocket",
        session_id,
        &ctx.auth.user_id,
        text,
    );

    let execution_id = Uuid::new_v4();

    // Spawn orchestrator processing in background
    let orchestrator = ctx.orchestrator.clone();
    let event_bus = ctx.event_bus.clone();
    let exec_id = execution_id;

    tokio::spawn(async move {
        match orchestrator.process(input).await {
            Ok(result) => {
                // Emit final ChatDelta with the complete response
                event_bus.publish(cratos_core::OrchestratorEvent::ChatDelta {
                    execution_id: result.execution_id,
                    delta: result.response,
                    is_final: true,
                });
            }
            Err(e) => {
                tracing::error!(execution_id = %exec_id, error = %e, "WS chat.send execution failed");
                event_bus.publish(cratos_core::OrchestratorEvent::ExecutionFailed {
                    execution_id: exec_id,
                    error: e.to_string(),
                });
            }
        }
    });

    GatewayFrame::ok(
        id,
        serde_json::json!({
            "execution_id": execution_id,
            "status": "accepted"
        }),
    )
}

async fn cancel(id: &str, params: serde_json::Value, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ExecutionWrite) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires ExecutionWrite scope"),
        );
    }

    let execution_id_str = params
        .get("execution_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let execution_id = match Uuid::parse_str(execution_id_str) {
        Ok(id) => id,
        Err(_) => {
            return GatewayFrame::err(
                id,
                GatewayError::new(
                    GatewayErrorCode::InvalidParams,
                    "Invalid or missing 'execution_id'",
                ),
            );
        }
    };

    let cancelled = ctx.orchestrator.cancel_execution(execution_id);
    GatewayFrame::ok(
        id,
        serde_json::json!({
            "execution_id": execution_id,
            "cancelled": cancelled
        }),
    )
}

#[cfg(test)]
mod tests {
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

    fn test_node_registry() -> NodeRegistry {
        NodeRegistry::new()
    }

    fn test_a2a_router() -> A2aRouter {
        A2aRouter::new(100)
    }

    fn test_browser_relay() -> crate::websocket::gateway::browser_relay::SharedBrowserRelay {
        Arc::new(BrowserRelay::new())
    }

    fn test_orchestrator() -> Arc<Orchestrator> {
        let provider: Arc<dyn cratos_llm::LlmProvider> = Arc::new(cratos_llm::MockProvider::new());
        let registry = Arc::new(ToolRegistry::new());
        Arc::new(Orchestrator::new(provider, registry, OrchestratorConfig::default()))
    }

    fn test_event_bus() -> Arc<EventBus> {
        Arc::new(EventBus::new(16))
    }

    #[tokio::test]
    async fn test_chat_send_with_scope() {
        let auth = admin_auth();
        let nr = test_node_registry();
        let a2a = test_a2a_router();
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
        let result = super::handle("2", "chat.send", serde_json::json!({"text": "hello"}), &ctx).await;
        match result {
            GatewayFrame::Response { result: Some(v), .. } => {
                assert_eq!(v["status"], "accepted");
                assert!(v["execution_id"].is_string());
            }
            _ => panic!("expected ok response"),
        }
    }

    #[tokio::test]
    async fn test_chat_send_without_scope() {
        let auth = readonly_auth();
        let nr = test_node_registry();
        let a2a = test_a2a_router();
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
        let result = super::handle("3", "chat.send", serde_json::json!({"text": "hello"}), &ctx).await;
        match result {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::Forbidden);
            }
            _ => panic!("expected error response"),
        }
    }

    #[tokio::test]
    async fn test_chat_send_missing_text() {
        let auth = admin_auth();
        let nr = test_node_registry();
        let a2a = test_a2a_router();
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
        let result = super::handle("4", "chat.send", serde_json::json!({}), &ctx).await;
        match result {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::InvalidParams);
            }
            _ => panic!("expected error response"),
        }
    }

    #[tokio::test]
    async fn test_chat_cancel_invalid_id() {
        let auth = admin_auth();
        let nr = test_node_registry();
        let a2a = test_a2a_router();
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
        let result = super::handle("5", "chat.cancel", serde_json::json!({"execution_id": "not-a-uuid"}), &ctx).await;
        match result {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::InvalidParams);
            }
            _ => panic!("expected error response"),
        }
    }
}
