//! Method dispatch routing for the Gateway WebSocket.

use cratos_core::{
    a2a::A2aRouter,
    auth::AuthContext,
    nodes::NodeRegistry,
};
use uuid::Uuid;

use crate::websocket::protocol::{GatewayError, GatewayErrorCode, GatewayFrame};

use super::handlers;

/// Shared context for method dispatch, replacing 6 individual parameters.
pub(crate) struct DispatchContext<'a> {
    pub auth: &'a AuthContext,
    pub node_registry: &'a NodeRegistry,
    pub a2a_router: &'a A2aRouter,
}

/// Route a method call to the appropriate handler.
pub(crate) async fn dispatch_method(
    id: &str,
    method: &str,
    params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    match method {
        "ping" => GatewayFrame::ok(id, serde_json::json!({"pong": true})),
        m if m.starts_with("chat.") => handlers::chat::handle(id, m, params, ctx).await,
        m if m.starts_with("session.") => handlers::session::handle(id, m, params, ctx).await,
        m if m.starts_with("approval.") => handlers::approval::handle(id, m, params, ctx).await,
        m if m.starts_with("node.") => handlers::node::handle(id, m, params, ctx).await,
        m if m.starts_with("a2a.") => handlers::a2a::handle(id, m, params, ctx).await,
        _ => GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::UnknownMethod,
                format!("Unknown method: {}", method),
            ),
        ),
    }
}

/// Public entry point for dispatching methods (used by ACP bridge).
pub async fn dispatch_method_public(
    id: &str,
    method: &str,
    params: serde_json::Value,
    auth: &AuthContext,
    node_registry: &NodeRegistry,
    a2a_router: &A2aRouter,
) -> GatewayFrame {
    let ctx = DispatchContext {
        auth,
        node_registry,
        a2a_router,
    };
    dispatch_method(id, method, params, &ctx).await
}

/// Parse a UUID parameter from a JSON value.
pub(crate) fn parse_uuid_param(
    params: &serde_json::Value,
    field: &str,
) -> Result<Uuid, GatewayFrame> {
    let value = params.get(field).and_then(|v| v.as_str()).unwrap_or("");
    Uuid::parse_str(value).map_err(|_| {
        GatewayFrame::err(
            "",
            GatewayError::new(
                GatewayErrorCode::InvalidParams,
                format!("Invalid or missing '{}' UUID", field),
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cratos_core::auth::AuthMethod;
    use cratos_core::auth::Scope;

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

    fn test_node_registry() -> NodeRegistry {
        NodeRegistry::new()
    }

    fn test_a2a_router() -> A2aRouter {
        A2aRouter::new(100)
    }

    #[tokio::test]
    async fn test_ping() {
        let nr = test_node_registry();
        let a2a = test_a2a_router();
        let ctx = DispatchContext {
            auth: &admin_auth(),
            node_registry: &nr,
            a2a_router: &a2a,
        };
        let result = dispatch_method("1", "ping", serde_json::json!({}), &ctx).await;
        match result {
            GatewayFrame::Response {
                result: Some(v), ..
            } => {
                assert_eq!(v["pong"], true);
            }
            _ => panic!("expected ok response"),
        }
    }

    #[tokio::test]
    async fn test_unknown_method() {
        let nr = test_node_registry();
        let a2a = test_a2a_router();
        let ctx = DispatchContext {
            auth: &admin_auth(),
            node_registry: &nr,
            a2a_router: &a2a,
        };
        let result = dispatch_method("5", "unknown.method", serde_json::json!({}), &ctx).await;
        match result {
            GatewayFrame::Response {
                error: Some(e), ..
            } => {
                assert_eq!(e.code, GatewayErrorCode::UnknownMethod);
            }
            _ => panic!("expected error response"),
        }
    }
}
