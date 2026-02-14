//! Tools API endpoints
//!
//! GET /api/v1/tools - List all available tools (from ToolRegistry)

use axum::{routing::get, Extension, Json, Router};
use cratos_tools::ToolRegistry;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

use super::config::ApiResponse;
use crate::middleware::auth::RequireAuth;

/// Tool information for API response
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ToolInfo {
    /// Tool name (identifier)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Tool category
    pub category: String,
    /// Whether the tool requires approval
    pub requires_approval: bool,
    /// Parameter schema (JSON Schema)
    pub parameters: serde_json::Value,
}

/// List all available tools from the ToolRegistry
#[utoipa::path(
    get,
    path = "/api/v1/tools",
    tag = "tools",
    responses(
        (status = 200, description = "List of available tools", body = Vec<ToolInfo>),
        (status = 401, description = "Unauthorized")
    ),
    security(("api_key" = []))
)]
pub async fn list_tools(
    RequireAuth(_auth): RequireAuth,
    registry: Option<Extension<Arc<ToolRegistry>>>,
) -> Json<ApiResponse<Vec<ToolInfo>>> {
    let tools: Vec<ToolInfo> = match registry {
        Some(Extension(reg)) => reg
            .list_definitions()
            .into_iter()
            .map(|def| ToolInfo {
                name: def.name.clone(),
                description: def.description.clone(),
                category: format!("{:?}", def.category),
                requires_approval: def.risk_level != cratos_tools::RiskLevel::Low,
                parameters: def.parameters.clone(),
            })
            .collect(),
        None => Vec::new(),
    };
    Json(ApiResponse::success(tools))
}

/// Create tools routes
pub fn tools_routes() -> Router {
    Router::new().route("/api/v1/tools", get(list_tools))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cratos_core::auth::{AuthContext, AuthMethod, Scope};

    fn test_auth() -> RequireAuth {
        RequireAuth(AuthContext {
            user_id: "test".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::Admin],
            session_id: None,
            device_id: None,
        })
    }

    #[tokio::test]
    async fn test_list_tools_no_registry() {
        let response = list_tools(test_auth(), None).await;
        assert!(response.0.success);
        let tools = response.0.data.unwrap();
        assert!(tools.is_empty());
    }
}
