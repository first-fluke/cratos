//! Scheduler API endpoints
//!
//! GET    /api/v1/scheduler/tasks - List scheduled tasks
//! POST   /api/v1/scheduler/tasks - Create a new task
//! GET    /api/v1/scheduler/tasks/:id - Get task details
//! PUT    /api/v1/scheduler/tasks/:id - Update a task
//! DELETE /api/v1/scheduler/tasks/:id - Delete a task

use axum::{extract::Path, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::config::ApiResponse;

/// Task view for API responses
#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub trigger_type: String,
    pub trigger_config: serde_json::Value,
    pub action_type: String,
    pub action_config: serde_json::Value,
    pub enabled: bool,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub run_count: i64,
    pub failure_count: i64,
}

/// Request to create a new task
#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub description: Option<String>,
    pub trigger_type: String,
    pub trigger_config: serde_json::Value,
    pub action_type: String,
    pub action_config: serde_json::Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
}

fn default_true() -> bool {
    true
}

/// Request to update a task
#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub trigger_config: Option<serde_json::Value>,
    pub action_config: Option<serde_json::Value>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
}

/// List all scheduled tasks
async fn list_tasks() -> Json<ApiResponse<Vec<TaskView>>> {
    // In production, this would query the SchedulerStore
    // For now, return mock data
    let tasks = vec![
        TaskView {
            id: Uuid::new_v4(),
            name: "Daily Backup Reminder".to_string(),
            description: Some("Send reminder at 9 AM".to_string()),
            trigger_type: "cron".to_string(),
            trigger_config: serde_json::json!({"expression": "0 9 * * *"}),
            action_type: "notification".to_string(),
            action_config: serde_json::json!({
                "channel": "telegram",
                "message": "Time to backup!"
            }),
            enabled: true,
            priority: 0,
            created_at: Utc::now() - chrono::Duration::days(7),
            last_run_at: Some(Utc::now() - chrono::Duration::hours(15)),
            next_run_at: Some(Utc::now() + chrono::Duration::hours(9)),
            run_count: 7,
            failure_count: 0,
        },
        TaskView {
            id: Uuid::new_v4(),
            name: "Hourly Health Check".to_string(),
            description: Some("Check system health every hour".to_string()),
            trigger_type: "interval".to_string(),
            trigger_config: serde_json::json!({"seconds": 3600}),
            action_type: "natural_language".to_string(),
            action_config: serde_json::json!({
                "prompt": "Check system health and report any issues"
            }),
            enabled: true,
            priority: 1,
            created_at: Utc::now() - chrono::Duration::days(3),
            last_run_at: Some(Utc::now() - chrono::Duration::minutes(45)),
            next_run_at: Some(Utc::now() + chrono::Duration::minutes(15)),
            run_count: 72,
            failure_count: 2,
        },
    ];

    Json(ApiResponse::success(tasks))
}

/// Create a new scheduled task
async fn create_task(Json(request): Json<CreateTaskRequest>) -> Json<ApiResponse<TaskView>> {
    // Validate trigger type
    if !is_valid_trigger_type(&request.trigger_type) {
        return Json(ApiResponse::error(format!(
            "Invalid trigger type: {}",
            request.trigger_type
        )));
    }

    // Validate action type
    if !is_valid_action_type(&request.action_type) {
        return Json(ApiResponse::error(format!(
            "Invalid action type: {}",
            request.action_type
        )));
    }

    // Create task (mock)
    let task = TaskView {
        id: Uuid::new_v4(),
        name: request.name,
        description: request.description,
        trigger_type: request.trigger_type,
        trigger_config: request.trigger_config,
        action_type: request.action_type,
        action_config: request.action_config,
        enabled: request.enabled,
        priority: request.priority,
        created_at: Utc::now(),
        last_run_at: None,
        next_run_at: Some(Utc::now() + chrono::Duration::hours(1)),
        run_count: 0,
        failure_count: 0,
    };

    Json(ApiResponse::success(task))
}

/// Get task details
async fn get_task(Path(id): Path<Uuid>) -> Json<ApiResponse<TaskView>> {
    // In production, this would query the SchedulerStore
    let task = TaskView {
        id,
        name: "Example Task".to_string(),
        description: Some("Example description".to_string()),
        trigger_type: "interval".to_string(),
        trigger_config: serde_json::json!({"seconds": 3600}),
        action_type: "natural_language".to_string(),
        action_config: serde_json::json!({"prompt": "Do something"}),
        enabled: true,
        priority: 0,
        created_at: Utc::now() - chrono::Duration::days(1),
        last_run_at: Some(Utc::now() - chrono::Duration::hours(1)),
        next_run_at: Some(Utc::now() + chrono::Duration::hours(1)),
        run_count: 24,
        failure_count: 0,
    };

    Json(ApiResponse::success(task))
}

/// Update a task
async fn update_task(
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateTaskRequest>,
) -> Json<ApiResponse<TaskView>> {
    // In production, this would update via SchedulerStore
    let task = TaskView {
        id,
        name: request.name.unwrap_or_else(|| "Updated Task".to_string()),
        description: request.description,
        trigger_type: "interval".to_string(),
        trigger_config: request
            .trigger_config
            .unwrap_or_else(|| serde_json::json!({"seconds": 3600})),
        action_type: "natural_language".to_string(),
        action_config: request
            .action_config
            .unwrap_or_else(|| serde_json::json!({"prompt": "Do something"})),
        enabled: request.enabled.unwrap_or(true),
        priority: request.priority.unwrap_or(0),
        created_at: Utc::now() - chrono::Duration::days(1),
        last_run_at: Some(Utc::now() - chrono::Duration::hours(1)),
        next_run_at: Some(Utc::now() + chrono::Duration::hours(1)),
        run_count: 24,
        failure_count: 0,
    };

    Json(ApiResponse::success(task))
}

/// Delete a task
async fn delete_task(Path(id): Path<Uuid>) -> Json<ApiResponse<()>> {
    // In production, this would delete via SchedulerStore
    tracing::info!("Deleting task: {}", id);
    Json(ApiResponse::success(()))
}

fn is_valid_trigger_type(trigger_type: &str) -> bool {
    matches!(
        trigger_type,
        "cron" | "interval" | "one_time" | "file" | "system"
    )
}

fn is_valid_action_type(action_type: &str) -> bool {
    matches!(
        action_type,
        "natural_language" | "tool_call" | "notification" | "shell" | "webhook"
    )
}

/// Create scheduler routes
pub fn scheduler_routes() -> Router {
    Router::new()
        .route("/api/v1/scheduler/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/v1/scheduler/tasks/:id",
            get(get_task).put(update_task).delete(delete_task),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_tasks() {
        let response = list_tasks().await;
        assert!(response.0.success);
    }

    #[tokio::test]
    async fn test_create_task() {
        let request = CreateTaskRequest {
            name: "Test Task".to_string(),
            description: None,
            trigger_type: "interval".to_string(),
            trigger_config: serde_json::json!({"seconds": 60}),
            action_type: "notification".to_string(),
            action_config: serde_json::json!({"message": "Test"}),
            enabled: true,
            priority: 0,
        };
        let response = create_task(Json(request)).await;
        assert!(response.0.success);
    }

    #[test]
    fn test_valid_trigger_types() {
        assert!(is_valid_trigger_type("cron"));
        assert!(is_valid_trigger_type("interval"));
        assert!(!is_valid_trigger_type("invalid"));
    }

    #[test]
    fn test_valid_action_types() {
        assert!(is_valid_action_type("notification"));
        assert!(is_valid_action_type("natural_language"));
        assert!(!is_valid_action_type("invalid"));
    }
}
