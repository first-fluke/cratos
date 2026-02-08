use cratos_core::auth::Scope;

use crate::websocket::protocol::{GatewayError, GatewayErrorCode, GatewayFrame};
use super::super::dispatch::DispatchContext;

pub(crate) async fn handle(
    id: &str,
    method: &str,
    _params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    match method {
        "approval.respond" => respond(id, ctx).await,
        _ => GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::UnknownMethod,
                format!("Unknown method: {}", method),
            ),
        ),
    }
}

async fn respond(id: &str, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ApprovalRespond) {
        return GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::Forbidden,
                "Requires ApprovalRespond scope",
            ),
        );
    }
    GatewayFrame::ok(id, serde_json::json!({"accepted": true}))
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

    #[tokio::test]
    async fn test_approval_respond_scope() {
        let nr = NodeRegistry::new();
        let a2a = A2aRouter::new(100);

        let ro = readonly_auth();
        let ctx = DispatchContext { auth: &ro, node_registry: &nr, a2a_router: &a2a };
        let result = super::handle("9", "approval.respond", serde_json::json!({}), &ctx).await;
        match result {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::Forbidden);
            }
            _ => panic!("expected error"),
        }

        // Admin should work
        let admin = admin_auth();
        let ctx = DispatchContext { auth: &admin, node_registry: &nr, a2a_router: &a2a };
        let result = super::handle("10", "approval.respond", serde_json::json!({}), &ctx).await;
        assert!(matches!(result, GatewayFrame::Response { result: Some(_), .. }));
    }
}
