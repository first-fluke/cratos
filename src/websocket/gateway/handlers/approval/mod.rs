use cratos_core::auth::Scope;
use uuid::Uuid;

use super::super::dispatch::DispatchContext;
use crate::websocket::protocol::{GatewayError, GatewayErrorCode, GatewayFrame};

#[cfg(test)]
mod tests;

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
            return GatewayFrame::ok(id, serde_json::json!({"pending": [], "count": 0}));
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
