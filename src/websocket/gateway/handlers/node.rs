use cratos_core::auth::Scope;
use cratos_core::nodes::{NodeError, NodeRegisterParams};

use crate::websocket::protocol::{GatewayError, GatewayErrorCode, GatewayFrame};
use super::super::dispatch::{parse_uuid_param, DispatchContext};

pub(crate) async fn handle(
    id: &str,
    method: &str,
    params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    match method {
        "node.register" => register(id, params, ctx).await,
        "node.heartbeat" => heartbeat(id, params, ctx).await,
        "node.list" => list(id, ctx).await,
        "node.invoke" => invoke(id, params, ctx).await,
        "node.remove" => remove(id, params, ctx).await,
        _ => GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::UnknownMethod,
                format!("Unknown method: {}", method),
            ),
        ),
    }
}

async fn register(
    id: &str,
    params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::NodeManage) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires NodeManage scope"),
        );
    }
    let register_params: NodeRegisterParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return GatewayFrame::err(
                id,
                GatewayError::new(
                    GatewayErrorCode::InvalidParams,
                    format!("Invalid register params: {}", e),
                ),
            );
        }
    };
    match ctx.node_registry.register(register_params, ctx.auth).await {
        Ok(node) => GatewayFrame::ok(
            id,
            serde_json::json!({
                "node_id": node.id,
                "name": node.name,
                "status": node.status,
            }),
        ),
        Err(e) => GatewayFrame::err(id, node_error_to_gateway(e)),
    }
}

async fn heartbeat(
    id: &str,
    params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::NodeManage) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires NodeManage scope"),
        );
    }
    let node_id = match parse_uuid_param(&params, "node_id") {
        Ok(id) => id,
        Err(frame) => return frame,
    };
    match ctx.node_registry.heartbeat(node_id, ctx.auth).await {
        Ok(()) => GatewayFrame::ok(id, serde_json::json!({"ok": true})),
        Err(e) => GatewayFrame::err(id, node_error_to_gateway(e)),
    }
}

async fn list(id: &str, ctx: &DispatchContext<'_>) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::SessionRead) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires SessionRead scope"),
        );
    }
    let nodes = ctx.node_registry.list_nodes(ctx.auth).await;
    GatewayFrame::ok(id, serde_json::json!({"nodes": nodes}))
}

async fn invoke(
    id: &str,
    params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::ExecutionWrite) || !ctx.auth.has_scope(&Scope::NodeManage) {
        return GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::Forbidden,
                "Requires ExecutionWrite + NodeManage scopes",
            ),
        );
    }
    let node_id = match parse_uuid_param(&params, "node_id") {
        Ok(id) => id,
        Err(frame) => return frame,
    };
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if command.is_empty() {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::InvalidParams, "Missing 'command' parameter"),
        );
    }
    // Check tool policy before attempting invoke
    match ctx.node_registry.check_command(node_id, command, ctx.auth).await {
        Ok(()) => GatewayFrame::ok(
            id,
            serde_json::json!({
                "status": "accepted",
                "node_id": node_id,
                "command": command,
                "message": "Command accepted. Remote execution pending node agent connection."
            }),
        ),
        Err(e) => GatewayFrame::err(id, node_error_to_gateway(e)),
    }
}

async fn remove(
    id: &str,
    params: serde_json::Value,
    ctx: &DispatchContext<'_>,
) -> GatewayFrame {
    if !ctx.auth.has_scope(&Scope::NodeManage) {
        return GatewayFrame::err(
            id,
            GatewayError::new(GatewayErrorCode::Forbidden, "Requires NodeManage scope"),
        );
    }
    let node_id = match parse_uuid_param(&params, "node_id") {
        Ok(id) => id,
        Err(frame) => return frame,
    };
    match ctx.node_registry.remove(node_id, ctx.auth).await {
        Ok(()) => GatewayFrame::ok(id, serde_json::json!({"deleted": true})),
        Err(e) => GatewayFrame::err(id, node_error_to_gateway(e)),
    }
}

/// Convert a NodeError to a GatewayError.
fn node_error_to_gateway(err: NodeError) -> GatewayError {
    match err {
        NodeError::NotFound(_) => {
            GatewayError::new(GatewayErrorCode::InvalidParams, err.to_string())
        }
        NodeError::Offline(_) => {
            GatewayError::new(GatewayErrorCode::InvalidParams, err.to_string())
        }
        NodeError::PolicyDenied(_) => {
            GatewayError::new(GatewayErrorCode::Forbidden, err.to_string())
        }
        NodeError::Unauthorized => {
            GatewayError::new(GatewayErrorCode::Forbidden, err.to_string())
        }
        NodeError::SignatureInvalid(_) => {
            GatewayError::new(GatewayErrorCode::Unauthorized, err.to_string())
        }
        NodeError::SignatureMissing => {
            GatewayError::new(GatewayErrorCode::InvalidParams, err.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::dispatch::{dispatch_method, DispatchContext};
    use crate::websocket::gateway::browser_relay::BrowserRelay;
    use crate::websocket::protocol::{GatewayErrorCode, GatewayFrame};
    use cratos_core::a2a::A2aRouter;
    use cratos_core::auth::{AuthContext, AuthMethod, Scope};
    use cratos_core::nodes::NodeRegistry;
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

    #[tokio::test]
    async fn test_node_register() {
        let nr = NodeRegistry::new();
        let a2a = A2aRouter::new(100);
        let auth = admin_auth();
        let br = test_browser_relay();
        let ctx = DispatchContext { auth: &auth, node_registry: &nr, a2a_router: &a2a, browser_relay: &br };
        let result = dispatch_method(
            "20",
            "node.register",
            serde_json::json!({
                "name": "test-node",
                "platform": "darwin",
                "capabilities": ["rust"],
                "declared_commands": ["git", "cargo"]
            }),
            &ctx,
        )
        .await;
        match result {
            GatewayFrame::Response { result: Some(v), .. } => {
                assert!(v.get("node_id").is_some());
                assert_eq!(v["name"], "test-node");
            }
            _ => panic!("expected ok response"),
        }
    }

    #[tokio::test]
    async fn test_node_register_without_scope() {
        let nr = NodeRegistry::new();
        let a2a = A2aRouter::new(100);
        let auth = readonly_auth();
        let br = test_browser_relay();
        let ctx = DispatchContext { auth: &auth, node_registry: &nr, a2a_router: &a2a, browser_relay: &br };
        let result = dispatch_method(
            "21",
            "node.register",
            serde_json::json!({"name": "n", "platform": "linux"}),
            &ctx,
        )
        .await;
        assert!(matches!(result, GatewayFrame::Response { error: Some(_), .. }));
    }

    #[tokio::test]
    async fn test_node_list() {
        let nr = NodeRegistry::new();
        let a2a = A2aRouter::new(100);
        let auth = admin_auth();
        let br = test_browser_relay();
        let ctx = DispatchContext { auth: &auth, node_registry: &nr, a2a_router: &a2a, browser_relay: &br };

        // Register a node first
        let _ = dispatch_method(
            "30",
            "node.register",
            serde_json::json!({
                "name": "n1",
                "platform": "linux",
                "declared_commands": ["ls"]
            }),
            &ctx,
        )
        .await;

        let result = dispatch_method("31", "node.list", serde_json::json!({}), &ctx).await;
        match result {
            GatewayFrame::Response { result: Some(v), .. } => {
                let nodes = v["nodes"].as_array().unwrap();
                assert_eq!(nodes.len(), 1);
            }
            _ => panic!("expected ok response"),
        }
    }

    #[tokio::test]
    async fn test_node_invoke_policy_denied() {
        let nr = NodeRegistry::new();
        let a2a = A2aRouter::new(100);
        let auth = admin_auth();
        let br = test_browser_relay();
        let ctx = DispatchContext { auth: &auth, node_registry: &nr, a2a_router: &a2a, browser_relay: &br };

        // Register a node
        let reg_result = dispatch_method(
            "40",
            "node.register",
            serde_json::json!({
                "name": "n1",
                "platform": "linux",
                "declared_commands": ["git"]
            }),
            &ctx,
        )
        .await;
        let node_id = match reg_result {
            GatewayFrame::Response { result: Some(v), .. } => {
                v["node_id"].as_str().unwrap().to_string()
            }
            _ => panic!("expected ok"),
        };

        // Heartbeat to bring online
        let _ = dispatch_method(
            "41",
            "node.heartbeat",
            serde_json::json!({"node_id": node_id}),
            &ctx,
        )
        .await;

        // Try to invoke an undeclared command
        let result = dispatch_method(
            "42",
            "node.invoke",
            serde_json::json!({"node_id": node_id, "command": "rm -rf /"}),
            &ctx,
        )
        .await;
        match result {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::Forbidden);
            }
            _ => panic!("expected policy error"),
        }
    }
}
