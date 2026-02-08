//! Tools API endpoints
//!
//! GET /api/v1/tools - List all available tools

use axum::{routing::get, Json, Router};
use serde::Serialize;

use super::config::ApiResponse;
use crate::middleware::auth::RequireAuth;

/// Tool information for API response
#[derive(Debug, Clone, Serialize)]
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

/// List all available tools (requires authentication)
async fn list_tools(RequireAuth(_auth): RequireAuth) -> Json<ApiResponse<Vec<ToolInfo>>> {
    // Return built-in tools info
    // In production, this would be populated from the ToolRegistry
    let tools = vec![
        ToolInfo {
            name: "file_read".to_string(),
            description: "Read contents of a file".to_string(),
            category: "file".to_string(),
            requires_approval: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path to read"}
                },
                "required": ["path"]
            }),
        },
        ToolInfo {
            name: "file_write".to_string(),
            description: "Write contents to a file".to_string(),
            category: "file".to_string(),
            requires_approval: true,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path to write"},
                    "content": {"type": "string", "description": "Content to write"}
                },
                "required": ["path", "content"]
            }),
        },
        ToolInfo {
            name: "file_list".to_string(),
            description: "List files in a directory".to_string(),
            category: "file".to_string(),
            requires_approval: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory path"}
                },
                "required": ["path"]
            }),
        },
        ToolInfo {
            name: "http_get".to_string(),
            description: "Make an HTTP GET request".to_string(),
            category: "http".to_string(),
            requires_approval: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "URL to fetch"}
                },
                "required": ["url"]
            }),
        },
        ToolInfo {
            name: "http_post".to_string(),
            description: "Make an HTTP POST request".to_string(),
            category: "http".to_string(),
            requires_approval: true,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "URL to post to"},
                    "body": {"type": "object", "description": "Request body"}
                },
                "required": ["url"]
            }),
        },
        ToolInfo {
            name: "exec".to_string(),
            description: "Execute a shell command".to_string(),
            category: "system".to_string(),
            requires_approval: true,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Command to execute"},
                    "cwd": {"type": "string", "description": "Working directory"}
                },
                "required": ["command"]
            }),
        },
        ToolInfo {
            name: "git_status".to_string(),
            description: "Get git repository status".to_string(),
            category: "git".to_string(),
            requires_approval: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Repository path"}
                }
            }),
        },
        ToolInfo {
            name: "git_commit".to_string(),
            description: "Create a git commit".to_string(),
            category: "git".to_string(),
            requires_approval: true,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string", "description": "Commit message"},
                    "path": {"type": "string", "description": "Repository path"}
                },
                "required": ["message"]
            }),
        },
        ToolInfo {
            name: "wol".to_string(),
            description: "Wake-on-LAN - wake a device by MAC address".to_string(),
            category: "network".to_string(),
            requires_approval: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "mac": {"type": "string", "description": "MAC address (e.g., AA:BB:CC:DD:EE:FF)"},
                    "port": {"type": "integer", "description": "WoL port (default: 9)"}
                },
                "required": ["mac"]
            }),
        },
        ToolInfo {
            name: "config".to_string(),
            description: "Change Cratos configuration using natural language".to_string(),
            category: "system".to_string(),
            requires_approval: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Natural language configuration command"}
                },
                "required": ["command"]
            }),
        },
    ];

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
    async fn test_list_tools() {
        let response = list_tools(test_auth()).await;
        assert!(response.0.success);
        let tools = response.0.data.unwrap();
        assert!(!tools.is_empty());

        // Check that file_read exists
        let file_read = tools.iter().find(|t| t.name == "file_read");
        assert!(file_read.is_some());

        // Check that wol exists
        let wol = tools.iter().find(|t| t.name == "wol");
        assert!(wol.is_some());
    }
}
