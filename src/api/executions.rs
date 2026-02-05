//! Executions API endpoints
//!
//! GET /api/v1/executions - List recent executions
//! GET /api/v1/executions/:id - Get execution details

use axum::{
    extract::{Path, Query},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::config::ApiResponse;

/// Query parameters for listing executions
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub event_count: i32,
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
    pub tool_calls: Vec<ToolCallSummary>,
}

/// Event summary
#[derive(Debug, Clone, Serialize)]
pub struct EventSummary {
    pub sequence_num: i32,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub summary: Option<String>,
}

/// Tool call summary
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallSummary {
    pub tool_name: String,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
}

/// List recent executions
async fn list_executions(
    Query(query): Query<ListExecutionsQuery>,
) -> Json<ApiResponse<Vec<ExecutionSummary>>> {
    // In production, this would query the EventStore
    // For now, return mock data
    let executions = vec![
        ExecutionSummary {
            id: Uuid::new_v4(),
            channel_type: "telegram".to_string(),
            channel_id: "123456".to_string(),
            user_id: "user1".to_string(),
            input_text: "Help me write a function".to_string(),
            status: "completed".to_string(),
            created_at: Utc::now() - chrono::Duration::hours(1),
            completed_at: Some(Utc::now() - chrono::Duration::minutes(58)),
            event_count: 5,
        },
        ExecutionSummary {
            id: Uuid::new_v4(),
            channel_type: "slack".to_string(),
            channel_id: "C789".to_string(),
            user_id: "user2".to_string(),
            input_text: "Check system status".to_string(),
            status: "completed".to_string(),
            created_at: Utc::now() - chrono::Duration::hours(2),
            completed_at: Some(
                Utc::now() - chrono::Duration::hours(2) + chrono::Duration::seconds(30),
            ),
            event_count: 3,
        },
    ];

    // Apply filters
    let filtered: Vec<_> = executions
        .into_iter()
        .filter(|e| {
            query
                .channel
                .as_ref()
                .is_none_or(|c| &e.channel_type == c)
                && query.status.as_ref().is_none_or(|s| &e.status == s)
        })
        .take(query.limit as usize)
        .collect();

    Json(ApiResponse::success(filtered))
}

/// Get execution details by ID
async fn get_execution(Path(id): Path<Uuid>) -> Json<ApiResponse<ExecutionDetail>> {
    // In production, this would query the EventStore
    // For now, return mock data
    let detail = ExecutionDetail {
        id,
        channel_type: "telegram".to_string(),
        channel_id: "123456".to_string(),
        user_id: "user1".to_string(),
        thread_id: None,
        input_text: "Help me write a function".to_string(),
        output_text: Some("Here's a function that does X...".to_string()),
        status: "completed".to_string(),
        created_at: Utc::now() - chrono::Duration::hours(1),
        completed_at: Some(Utc::now() - chrono::Duration::minutes(58)),
        events: vec![
            EventSummary {
                sequence_num: 1,
                event_type: "user_input".to_string(),
                timestamp: Utc::now() - chrono::Duration::hours(1),
                summary: Some("User input received".to_string()),
            },
            EventSummary {
                sequence_num: 2,
                event_type: "llm_request".to_string(),
                timestamp: Utc::now() - chrono::Duration::hours(1) + chrono::Duration::seconds(1),
                summary: Some("Sent request to LLM".to_string()),
            },
            EventSummary {
                sequence_num: 3,
                event_type: "llm_response".to_string(),
                timestamp: Utc::now() - chrono::Duration::hours(1) + chrono::Duration::seconds(3),
                summary: Some("Received LLM response".to_string()),
            },
            EventSummary {
                sequence_num: 4,
                event_type: "tool_call".to_string(),
                timestamp: Utc::now() - chrono::Duration::hours(1) + chrono::Duration::seconds(4),
                summary: Some("Called file_write tool".to_string()),
            },
            EventSummary {
                sequence_num: 5,
                event_type: "final_response".to_string(),
                timestamp: Utc::now() - chrono::Duration::minutes(58),
                summary: Some("Sent final response".to_string()),
            },
        ],
        tool_calls: vec![ToolCallSummary {
            tool_name: "file_write".to_string(),
            status: "success".to_string(),
            duration_ms: Some(150),
            error: None,
        }],
    };

    Json(ApiResponse::success(detail))
}

/// Create executions routes
pub fn executions_routes() -> Router {
    Router::new()
        .route("/api/v1/executions", get(list_executions))
        .route("/api/v1/executions/:id", get(get_execution))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_executions() {
        let query = ListExecutionsQuery {
            limit: 10,
            channel: None,
            status: None,
            from: None,
            to: None,
        };
        let response = list_executions(Query(query)).await;
        assert!(response.0.success);
    }

    #[tokio::test]
    async fn test_get_execution() {
        let id = Uuid::new_v4();
        let response = get_execution(Path(id)).await;
        assert!(response.0.success);
        let detail = response.0.data.unwrap();
        assert_eq!(detail.id, id);
    }
}
