use cratos_core::auth::Scope;
use uuid::Uuid;

use crate::websocket::protocol::{GatewayError, GatewayErrorCode, GatewayFrame};
use super::super::dispatch::DispatchContext;

pub(crate) async fn handle(
    id: &str,
    method: &str,
    _params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    match method {
        "session.list" => list(id, ctx).await,
        "session.create" => create(id, ctx).await,
        "session.delete" => delete(id, ctx).await,
        _ => GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::UnknownMethod,
                format!("Unknown method: {}", method),
            ),
        ),
    }
}

async fn list(id: &str, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::SessionRead) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires SessionRead scope"),
        );
    }
    GatewayFrame::ok(id, serde_json::json!({"sessions": []}))
}

async fn create(id: &str, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::SessionWrite) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires SessionWrite scope"),
        );
    }
    let session_id = Uuid::new_v4();
    GatewayFrame::ok(id, serde_json::json!({"session_id": session_id}))
}

async fn delete(id: &str, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::SessionWrite) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires SessionWrite scope"),
        );
    }
    GatewayFrame::ok(id, serde_json::json!({"deleted": true}))
}

#[cfg(test)]
mod tests {
    use super::super::super::dispatch::DispatchContext;
    use crate::websocket::gateway::browser_relay::BrowserRelay;
    use crate::websocket::protocol::GatewayFrame;
    use cratos_core::a2a::A2aRouter;
    use cratos_core::auth::{AuthContext, AuthMethod, Scope};
    use cratos_core::nodes::NodeRegistry;
    use std::sync::Arc;

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

    #[tokio::test]
    async fn test_session_operations_scope_check() {
        let nr = NodeRegistry::new();
        let a2a = A2aRouter::new(100);
        let ro = readonly_auth();
        let br = test_browser_relay();
        let ctx = DispatchContext { auth: &ro, node_registry: &nr, a2a_router: &a2a, browser_relay: &br };

        // session.list should work with SessionRead
        let result = super::handle("6", "session.list", serde_json::json!({}), &ctx).await;
        assert!(matches!(result, GatewayFrame::Response { result: Some(_), .. }));

        // session.create should fail without SessionWrite
        let result = super::handle("7", "session.create", serde_json::json!({}), &ctx).await;
        assert!(matches!(result, GatewayFrame::Response { error: Some(_), .. }));

        // session.delete should fail without SessionWrite
        let result = super::handle("8", "session.delete", serde_json::json!({}), &ctx).await;
        assert!(matches!(result, GatewayFrame::Response { error: Some(_), .. }));
    }
}
