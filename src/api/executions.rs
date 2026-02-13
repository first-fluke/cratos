//! Executions API endpoints
//!
//! GET /api/v1/executions - List recent executions
//! GET /api/v1/executions/:id - Get execution details

use std::sync::Arc;

use axum::{
    extract::{Path, Query},
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use cratos_replay::{EventStore, ExecutionViewer, ReplayOptions};

use super::config::ApiResponse;
use crate::middleware::auth::RequireAuth;

/// Query parameters for listing executions
#[derive(Debug, Deserialize)]
pub struct ListExecutionsQuery {
    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Filter by channel type
    pub channel: Option<String>,
    /// Filter by status
    pub status: Option<String>,
    /// Filter by date (from)
    pub from: Option<DateTime<Utc>>,
    /// Filter by date (to)
    pub to: Option<DateTime<Utc>>,
}

fn default_limit() -> i64 {
    50
}

/// Execution summary for list view
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionSummary {
    pub id: Uuid,
    pub channel_type: String,
    pub channel_id: String,
    pub user_id: String,
    pub input_text: String,
    pub output_text: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Detailed execution view
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionDetail {
    pub id: Uuid,
    pub channel_type: String,
    pub channel_id: String,
    pub user_id: String,
    pub thread_id: Option<String>,
    pub input_text: String,
    pub output_text: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub events: Vec<EventSummary>,
}

/// Event summary
#[derive(Debug, Clone, Serialize)]
pub struct EventSummary {
    pub id: Uuid,
    pub sequence_num: i32,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: Option<i32>,
}

/// List recent executions (requires authentication)
async fn list_executions(
    RequireAuth(_auth): RequireAuth,
    Extension(store): Extension<Arc<EventStore>>,
    Query(query): Query<ListExecutionsQuery>,
) -> Json<ApiResponse<Vec<ExecutionSummary>>> {
    let limit = query.limit.clamp(1, 200);

    // Use channel-specific query if channel filter provided
    let executions = if let Some(ref channel) = query.channel {
        match store
            .list_executions_by_channel(channel, "", limit, 0)
            .await
        {
            Ok(execs) => execs,
            Err(e) => {
                return Json(ApiResponse::error(format!(
                    "Failed to list executions: {}",
                    e
                )));
            }
        }
    } else {
        match store.list_recent_executions(limit).await {
            Ok(execs) => execs,
            Err(e) => {
                return Json(ApiResponse::error(format!(
                    "Failed to list executions: {}",
                    e
                )));
            }
        }
    };

    // Apply additional filters in memory
    let summaries: Vec<ExecutionSummary> = executions
        .into_iter()
        .filter(|e| {
            query
                .status
                .as_ref()
                .is_none_or(|s| e.status.to_string() == *s)
        })
        .filter(|e| query.from.is_none_or(|from| e.created_at >= from))
        .filter(|e| query.to.is_none_or(|to| e.created_at <= to))
        .map(|e| ExecutionSummary {
            id: e.id,
            channel_type: e.channel_type,
            channel_id: e.channel_id,
            user_id: e.user_id,
            input_text: e.input_text,
            output_text: e.output_text,
            status: e.status.to_string(),
            created_at: e.created_at,
            completed_at: e.completed_at,
        })
        .collect();

    Json(ApiResponse::success(summaries))
}

/// Get execution details by ID (requires authentication)
async fn get_execution(
    RequireAuth(_auth): RequireAuth,
    Extension(store): Extension<Arc<EventStore>>,
    Path(id): Path<Uuid>,
) -> Json<ApiResponse<ExecutionDetail>> {
    // Fetch execution
    let execution = match store.get_execution(id).await {
        Ok(e) => e,
        Err(e) => {
            return Json(ApiResponse::error(format!("Execution not found: {}", e)));
        }
    };

    // Fetch associated events
    let events = match store.get_execution_events(id).await {
        Ok(evts) => evts,
        Err(e) => {
            return Json(ApiResponse::error(format!("Failed to load events: {}", e)));
        }
    };

    let event_summaries: Vec<EventSummary> = events
        .into_iter()
        .map(|ev| EventSummary {
            id: ev.id,
            sequence_num: ev.sequence_num,
            event_type: ev.event_type.as_str().to_string(),
            timestamp: ev.timestamp,
            duration_ms: ev.duration_ms,
        })
        .collect();

    let detail = ExecutionDetail {
        id: execution.id,
        channel_type: execution.channel_type,
        channel_id: execution.channel_id,
        user_id: execution.user_id,
        thread_id: execution.thread_id,
        input_text: execution.input_text,
        output_text: execution.output_text,
        status: execution.status.to_string(),
        created_at: execution.created_at,
        completed_at: execution.completed_at,
        events: event_summaries,
    };

    Json(ApiResponse::success(detail))
}

/// Get replay timeline events for an execution (requires authentication)
async fn get_replay_events(
    RequireAuth(_auth): RequireAuth,
    Extension(store): Extension<Arc<EventStore>>,
    Path(id): Path<Uuid>,
) -> Json<ApiResponse<Vec<cratos_replay::TimelineEntry>>> {
    let viewer = ExecutionViewer::new((*store).clone());
    match viewer.get_timeline(id).await {
        Ok(timeline) => Json(ApiResponse::success(timeline)),
        Err(e) => Json(ApiResponse::error(format!(
            "Failed to get replay events: {}",
            e
        ))),
    }
}

/// Rerun an execution with replay options (requires authentication)
async fn rerun_execution(
    RequireAuth(_auth): RequireAuth,
    Extension(store): Extension<Arc<EventStore>>,
    Path(id): Path<Uuid>,
    Json(options): Json<ReplayOptions>,
) -> Json<ApiResponse<cratos_replay::ReplayResult>> {
    let viewer = ExecutionViewer::new((*store).clone());
    match viewer.rerun(id, options).await {
        Ok(result) => Json(ApiResponse::success(result)),
        Err(e) => Json(ApiResponse::error(format!(
            "Failed to rerun execution: {}",
            e
        ))),
    }
}

/// Create executions routes
pub fn executions_routes() -> Router {
    Router::new()
        .route("/api/v1/executions", get(list_executions))
        .route("/api/v1/executions/:id", get(get_execution))
        .route("/api/v1/executions/:id/replay", get(get_replay_events))
        .route("/api/v1/executions/:id/rerun", post(rerun_execution))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_summary_serialization() {
        let summary = ExecutionSummary {
            id: Uuid::nil(),
            channel_type: "telegram".to_string(),
            channel_id: "123".to_string(),
            user_id: "user1".to_string(),
            input_text: "hello".to_string(),
            output_text: Some("world".to_string()),
            status: "completed".to_string(),
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"channel_type\":\"telegram\""));
        assert!(json.contains("\"status\":\"completed\""));
    }

    #[test]
    fn test_execution_detail_serialization() {
        let detail = ExecutionDetail {
            id: Uuid::nil(),
            channel_type: "websocket".to_string(),
            channel_id: "ws1".to_string(),
            user_id: "user1".to_string(),
            thread_id: None,
            input_text: "test input".to_string(),
            output_text: Some("test output".to_string()),
            status: "completed".to_string(),
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
            events: vec![EventSummary {
                id: Uuid::nil(),
                sequence_num: 1,
                event_type: "user_input".to_string(),
                timestamp: Utc::now(),
                duration_ms: None,
            }],
        };
        let json = serde_json::to_string(&detail).unwrap();
        assert!(json.contains("\"events\""));
        assert!(json.contains("\"user_input\""));
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 50);
    }

    #[test]
    fn test_list_query_deserialization() {
        let json = r#"{"limit": 10, "channel": "telegram"}"#;
        let query: ListExecutionsQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.limit, 10);
        assert_eq!(query.channel.as_deref(), Some("telegram"));
    }

    #[test]
    fn test_replay_options_deserialization() {
        let json = r#"{"dry_run": true, "skip_tools": ["exec"]}"#;
        let opts: cratos_replay::ReplayOptions = serde_json::from_str(json).unwrap();
        assert!(opts.dry_run);
        assert_eq!(opts.skip_tools, vec!["exec"]);
    }

    #[test]
    fn test_replay_result_serialization() {
        let result = cratos_replay::ReplayResult {
            original_execution_id: Uuid::nil(),
            new_execution_id: None,
            steps: vec![],
            dry_run: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"dry_run\":true"));
    }
}
