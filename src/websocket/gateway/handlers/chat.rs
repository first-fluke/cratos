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
        "chat.send" => send(id, params, ctx).await,
        "chat.cancel" => cancel(id, ctx).await,
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
    let execution_id = Uuid::new_v4();
    GatewayFrame::ok(
        id,
        serde_json::json!({
            "execution_id": execution_id,
            "status": "accepted"
        }),
    )
}

async fn cancel(id: &str, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ExecutionWrite) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires ExecutionWrite scope"),
        );
    }
    GatewayFrame::ok(id, serde_json::json!({"cancelled": true}))
}

#[cfg(test)]
mod tests {
    use super::super::super::dispatch::DispatchContext;
    use crate::websocket::protocol::{GatewayErrorCode, GatewayFrame};
    use cratos_core::a2a::A2aRouter;
    use cratos_core::auth::{AuthContext, AuthMethod, Scope};
    use cratos_core::nodes::NodeRegistry;

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

    #[tokio::test]
    async fn test_chat_send_with_scope() {
        let auth = admin_auth();
        let nr = test_node_registry();
        let a2a = test_a2a_router();
        let ctx = DispatchContext { auth: &auth, node_registry: &nr, a2a_router: &a2a };
        let result = super::handle("2", "chat.send", serde_json::json!({"text": "hello"}), &ctx).await;
        match result {
            GatewayFrame::Response { result: Some(v), .. } => {
                assert_eq!(v["status"], "accepted");
            }
            _ => panic!("expected ok response"),
        }
    }

    #[tokio::test]
    async fn test_chat_send_without_scope() {
        let auth = readonly_auth();
        let nr = test_node_registry();
        let a2a = test_a2a_router();
        let ctx = DispatchContext { auth: &auth, node_registry: &nr, a2a_router: &a2a };
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
        let ctx = DispatchContext { auth: &auth, node_registry: &nr, a2a_router: &a2a };
        let result = super::handle("4", "chat.send", serde_json::json!({}), &ctx).await;
        match result {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::InvalidParams);
            }
            _ => panic!("expected error response"),
        }
    }
}
