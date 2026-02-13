//! Dev Sessions API endpoints
//!
//! GET /api/v1/dev/sessions - List all active AI dev sessions
//! GET /api/v1/dev/sessions/:tool - List sessions for a specific tool

use axum::{extract::Path, routing::get, Extension, Json, Router};
use cratos_core::dev_sessions::{DevSessionMonitor, DevTool};
use serde_json::Value;
use std::sync::Arc;

/// Create the dev sessions routes
pub fn dev_sessions_routes() -> Router {
    Router::new()
        .route("/api/v1/dev/sessions", get(list_sessions))
        .route("/api/v1/dev/sessions/{tool}", get(list_sessions_by_tool))
}

async fn list_sessions(Extension(monitor): Extension<Arc<DevSessionMonitor>>) -> Json<Value> {
    let sessions = monitor.sessions().await;
    Json(serde_json::json!({
        "sessions": sessions,
        "count": sessions.len(),
    }))
}

async fn list_sessions_by_tool(
    Extension(monitor): Extension<Arc<DevSessionMonitor>>,
    Path(tool_name): Path<String>,
) -> Json<Value> {
    let tool = match tool_name.as_str() {
        "claude" | "claude_code" | "claude-code" => Some(DevTool::ClaudeCode),
        "gemini" | "gemini_cli" | "gemini-cli" => Some(DevTool::GeminiCli),
        "codex" => Some(DevTool::Codex),
        "cursor" => Some(DevTool::Cursor),
        _ => None,
    };

    match tool {
        Some(t) => {
            let sessions = monitor.sessions_for_tool(t).await;
            Json(serde_json::json!({
                "tool": tool_name,
                "sessions": sessions,
                "count": sessions.len(),
            }))
        }
        None => Json(serde_json::json!({
            "error": format!("Unknown tool: {}. Valid: claude, gemini, codex, cursor", tool_name),
            "sessions": [],
            "count": 0,
        })),
    }
}
