use cratos_core::auth::Scope;
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
        "approval.respond" => respond(id, params, ctx).await,
        "approval.list" => list_pending(id, ctx).await,
        _ => GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::UnknownMethod,
                format!("Unknown method: {}", method),
            ),
        ),
    }
}

async fn respond(id: &str, params: serde_json::Value, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ApprovalRespond) {
        return GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::Forbidden,
                "Requires ApprovalRespond scope",
            ),
        );
    }

    let request_id_str = params
        .get("request_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let request_id = match Uuid::parse_str(request_id_str) {
        Ok(uuid) => uuid,
        Err(_) => {
            return GatewayFrame::err(
                id,
                GatewayError::new(
                    GatewayErrorCode::InvalidParams,
                    "Invalid or missing 'request_id'",
                ),
            );
        }
    };

    let approved = params
        .get("approved")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let manager = match ctx.approval_manager {
        Some(m) => m,
        None => {
            // No approval manager configured — accept anyway for backward compatibility
            return GatewayFrame::ok(
                id,
                serde_json::json!({
                    "request_id": request_id,
                    "accepted": approved,
                    "message": "Approval manager not configured"
                }),
            );
        }
    };

    let result = if approved {
        manager.approve_by(request_id, &ctx.auth.user_id).await
    } else {
        manager.reject_by(request_id, &ctx.auth.user_id).await
    };

    match result {
        Some(req) => GatewayFrame::ok(
            id,
            serde_json::json!({
                "request_id": request_id,
                "status": format!("{:?}", req.status),
                "accepted": approved,
            }),
        ),
        None => GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::InvalidParams,
                "Request not found, expired, or unauthorized",
            ),
        ),
    }
}

async fn list_pending(id: &str, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ApprovalRespond) {
        return GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::Forbidden,
                "Requires ApprovalRespond scope",
            ),
        );
    }

    let manager = match ctx.approval_manager {
        Some(m) => m,
        None => {
            return GatewayFrame::ok(
                id,
                serde_json::json!({"pending": [], "count": 0}),
            );
        }
    };

    let pending = manager.pending_for_user(&ctx.auth.user_id).await;
    let summaries: Vec<serde_json::Value> = pending
        .iter()
        .map(|r| {
            serde_json::json!({
                "request_id": r.id,
                "execution_id": r.execution_id,
                "action": r.action,
                "tool_name": r.tool_name,
                "created_at": r.created_at.to_rfc3339(),
                "expires_at": r.expires_at.to_rfc3339(),
            })
        })
        .collect();

    GatewayFrame::ok(
        id,
        serde_json::json!({
            "pending": summaries,
            "count": summaries.len(),
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
    async fn test_approval_respond_scope() {
        let nr = NodeRegistry::new();
        let a2a = A2aRouter::new(100);
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();

        // Readonly user should be forbidden
        let ro = readonly_auth();
        let ctx = DispatchContext { auth: &ro, node_registry: &nr, a2a_router: &a2a, browser_relay: &br, orchestrator: &orch, event_bus: &eb, approval_manager: None };
        let result = super::handle("9", "approval.respond", serde_json::json!({}), &ctx).await;
        match result {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::Forbidden);
            }
            _ => panic!("expected error"),
        }

        // Admin without approval_manager → returns ok with "not configured" message
        let admin = admin_auth();
        let ctx = DispatchContext { auth: &admin, node_registry: &nr, a2a_router: &a2a, browser_relay: &br, orchestrator: &orch, event_bus: &eb, approval_manager: None };
        let result = super::handle("10", "approval.respond", serde_json::json!({"request_id": "00000000-0000-0000-0000-000000000000", "approved": true}), &ctx).await;
        assert!(matches!(result, GatewayFrame::Response { result: Some(_), .. }));
    }

    #[tokio::test]
    async fn test_approval_respond_invalid_request_id() {
        let nr = NodeRegistry::new();
        let a2a = A2aRouter::new(100);
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();
        let admin = admin_auth();
        let ctx = DispatchContext { auth: &admin, node_registry: &nr, a2a_router: &a2a, browser_relay: &br, orchestrator: &orch, event_bus: &eb, approval_manager: None };
        let result = super::handle("11", "approval.respond", serde_json::json!({"request_id": "not-a-uuid"}), &ctx).await;
        match result {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::InvalidParams);
            }
            _ => panic!("expected error"),
        }
    }

    #[tokio::test]
    async fn test_approval_list_pending() {
        let nr = NodeRegistry::new();
        let a2a = A2aRouter::new(100);
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();
        let admin = admin_auth();
        let ctx = DispatchContext { auth: &admin, node_registry: &nr, a2a_router: &a2a, browser_relay: &br, orchestrator: &orch, event_bus: &eb, approval_manager: None };
        let result = super::handle("12", "approval.list", serde_json::json!({}), &ctx).await;
        match result {
            GatewayFrame::Response { result: Some(v), .. } => {
                assert_eq!(v["count"], 0);
            }
            _ => panic!("expected ok"),
        }
    }
}
