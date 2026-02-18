//! Scheduler API endpoints
//!
//! GET    /api/v1/scheduler/tasks - List scheduled tasks
//! POST   /api/v1/scheduler/tasks - Create a new task
//! GET    /api/v1/scheduler/tasks/:id - Get task details
//! PUT    /api/v1/scheduler/tasks/:id - Update a task
//! DELETE /api/v1/scheduler/tasks/:id - Delete a task

use std::sync::Arc;

use axum::{extract::Path, routing::get, Extension, Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use cratos_core::scheduler::{ScheduledTask, SchedulerEngine, TaskAction, TriggerType};

use super::config::ApiResponse;
use crate::middleware::auth::{require_scope, RequireAuth};

/// Task view for API responses
#[derive(Debug, Clone, Serialize, ToSchema)]
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
#[derive(Debug, Deserialize, ToSchema)]
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
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct UpdateTaskRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub trigger_config: Option<serde_json::Value>,
    pub action_config: Option<serde_json::Value>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
}

/// Convert a ScheduledTask to a TaskView for API response
fn task_to_view(task: &ScheduledTask) -> TaskView {
    let (trigger_type, trigger_config) = match &task.trigger {
        TriggerType::Cron(c) => (
            "cron".to_string(),
            serde_json::json!({ "expression": c.expression, "timezone": c.timezone }),
        ),
        TriggerType::Interval(i) => (
            "interval".to_string(),
            serde_json::json!({ "seconds": i.seconds, "immediate": i.immediate }),
        ),
        TriggerType::OneTime(o) => ("one_time".to_string(), serde_json::json!({ "at": o.at })),
        TriggerType::File(f) => (
            "file".to_string(),
            serde_json::to_value(f).unwrap_or_default(),
        ),
        TriggerType::System(s) => (
            "system".to_string(),
            serde_json::to_value(s).unwrap_or_default(),
        ),
    };

    let (action_type, action_config) = match &task.action {
        TaskAction::NaturalLanguage { prompt, channel } => (
            "natural_language".to_string(),
            serde_json::json!({ "prompt": prompt, "channel": channel }),
        ),
        TaskAction::ToolCall { tool, args } => (
            "tool_call".to_string(),
            serde_json::json!({ "tool": tool, "args": args }),
        ),
        TaskAction::Notification {
            channel,
            channel_id,
            message,
        } => (
            "notification".to_string(),
            serde_json::json!({ "channel": channel, "channel_id": channel_id, "message": message }),
        ),
        TaskAction::Shell { command, cwd } => (
            "shell".to_string(),
            serde_json::json!({ "command": command, "cwd": cwd }),
        ),
        TaskAction::Webhook { .. } => (
            "webhook".to_string(),
            serde_json::to_value(&task.action).unwrap_or_default(),
        ),
        TaskAction::RunSkillAnalysis { dry_run } => (
            "run_skill_analysis".to_string(),
            serde_json::json!({ "dry_run": dry_run }),
        ),
    };

    TaskView {
        id: task.id,
        name: task.name.clone(),
        description: task.description.clone(),
        trigger_type,
        trigger_config,
        action_type,
        action_config,
        enabled: task.enabled,
        priority: task.priority,
        created_at: task.created_at,
        last_run_at: task.last_run_at,
        next_run_at: task.next_run_at,
        run_count: task.run_count,
        failure_count: task.failure_count,
    }
}

/// Parse trigger from API request
fn parse_trigger(trigger_type: &str, config: &serde_json::Value) -> Result<TriggerType, String> {
    match trigger_type {
        "cron" => {
            let expr = config["expression"]
                .as_str()
                .ok_or("Missing cron expression")?;
            Ok(TriggerType::cron(expr))
        }
        "interval" => {
            let seconds = config["seconds"]
                .as_u64()
                .ok_or("Missing interval seconds")?;
            Ok(TriggerType::interval(seconds))
        }
        "one_time" => {
            let at_str = config["at"].as_str().ok_or("Missing one_time 'at' field")?;
            let at = DateTime::parse_from_rfc3339(at_str)
                .map_err(|e| format!("Invalid datetime: {}", e))?
                .with_timezone(&Utc);
            Ok(TriggerType::one_time(at))
        }
        other => Err(format!("Invalid trigger type: {}", other)),
    }
}

/// Parse action from API request
fn parse_action(action_type: &str, config: &serde_json::Value) -> Result<TaskAction, String> {
    match action_type {
        "natural_language" => {
            let prompt = config["prompt"]
                .as_str()
                .ok_or("Missing prompt")?
                .to_string();
            let channel = config["channel"].as_str().map(String::from);
            Ok(TaskAction::NaturalLanguage { prompt, channel })
        }
        "tool_call" => {
            let tool = config["tool"]
                .as_str()
                .ok_or("Missing tool name")?
                .to_string();
            let args = config
                .get("args")
                .cloned()
                .unwrap_or(serde_json::Value::Object(Default::default()));
            Ok(TaskAction::ToolCall { tool, args })
        }
        "notification" => {
            let channel = config["channel"]
                .as_str()
                .ok_or("Missing channel")?
                .to_string();
            let channel_id = config["channel_id"].as_str().unwrap_or("").to_string();
            let message = config["message"]
                .as_str()
                .ok_or("Missing message")?
                .to_string();
            Ok(TaskAction::Notification {
                channel,
                channel_id,
                message,
            })
        }
        "shell" => {
            let command = config["command"]
                .as_str()
                .ok_or("Missing command")?
                .to_string();
            let cwd = config["cwd"].as_str().map(String::from);
            Ok(TaskAction::Shell { command, cwd })
        }
        "webhook" => serde_json::from_value(config.clone()).map_err(|e| e.to_string()),
        "run_skill_analysis" => {
            let dry_run = config["dry_run"].as_bool().unwrap_or(false);
            Ok(TaskAction::RunSkillAnalysis { dry_run })
        }
        other => Err(format!("Invalid action type: {}", other)),
    }
}

/// List all scheduled tasks (requires authentication + scheduler_read scope)
#[utoipa::path(
    get,
    path = "/api/v1/scheduler/tasks",
    tag = "scheduler",
    responses(
        (status = 200, description = "List of scheduled tasks", body = Vec<TaskView>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - missing SchedulerRead scope")
    ),
    security(("api_key" = []))
)]
pub async fn list_tasks(
    RequireAuth(auth): RequireAuth,
    engine: Option<Extension<Arc<SchedulerEngine>>>,
) -> Result<Json<ApiResponse<Vec<TaskView>>>, crate::middleware::auth::AuthRejection> {
    require_scope(&auth, &cratos_core::Scope::SchedulerRead)?;
    let Some(Extension(engine)) = engine else {
        return Ok(Json(ApiResponse::error("Scheduler not enabled")));
    };

    match engine.list_tasks().await {
        Ok(tasks) => {
            let views: Vec<TaskView> = tasks.iter().map(task_to_view).collect();
            Ok(Json(ApiResponse::success(views)))
        }
        Err(e) => Ok(Json(ApiResponse::error(format!(
            "Failed to list tasks: {}",
            e
        )))),
    }
}

/// Create a new scheduled task (requires authentication + scheduler_write scope)
#[utoipa::path(
    post,
    path = "/api/v1/scheduler/tasks",
    tag = "scheduler",
    request_body = CreateTaskRequest,
    responses(
        (status = 200, description = "Created task", body = TaskView),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - missing SchedulerWrite scope")
    ),
    security(("api_key" = []))
)]
pub async fn create_task(
    RequireAuth(auth): RequireAuth,
    engine: Option<Extension<Arc<SchedulerEngine>>>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<ApiResponse<TaskView>>, crate::middleware::auth::AuthRejection> {
    require_scope(&auth, &cratos_core::Scope::SchedulerWrite)?;
    let Some(Extension(engine)) = engine else {
        return Ok(Json(ApiResponse::error("Scheduler not enabled")));
    };

    // Parse trigger
    let trigger = match parse_trigger(&request.trigger_type, &request.trigger_config) {
        Ok(t) => t,
        Err(e) => return Ok(Json(ApiResponse::error(e))),
    };

    // Parse action
    let action = match parse_action(&request.action_type, &request.action_config) {
        Ok(a) => a,
        Err(e) => return Ok(Json(ApiResponse::error(e))),
    };

    // Build task
    let mut task =
        ScheduledTask::new(&request.name, trigger, action).with_priority(request.priority);
    if let Some(desc) = request.description {
        task = task.with_description(desc);
    }
    task.enabled = request.enabled;

    let view = task_to_view(&task);

    match engine.add_task(task).await {
        Ok(()) => {
            info!("Created scheduled task: {} ({})", view.name, view.id);
            Ok(Json(ApiResponse::success(view)))
        }
        Err(e) => Ok(Json(ApiResponse::error(format!(
            "Failed to create task: {}",
            e
        )))),
    }
}

/// Get task details (requires authentication + scheduler_read scope)
#[utoipa::path(
    get,
    path = "/api/v1/scheduler/tasks/{id}",
    tag = "scheduler",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task details", body = TaskView),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Task not found")
    ),
    security(("api_key" = []))
)]
pub async fn get_task(
    RequireAuth(auth): RequireAuth,
    engine: Option<Extension<Arc<SchedulerEngine>>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<TaskView>>, crate::middleware::auth::AuthRejection> {
    require_scope(&auth, &cratos_core::Scope::SchedulerRead)?;
    let Some(Extension(engine)) = engine else {
        return Ok(Json(ApiResponse::error("Scheduler not enabled")));
    };

    match engine.get_task(id).await {
        Ok(task) => Ok(Json(ApiResponse::success(task_to_view(&task)))),
        Err(e) => Ok(Json(ApiResponse::error(format!("Task not found: {}", e)))),
    }
}

/// Update a task (requires authentication + scheduler_write scope)
#[utoipa::path(
    put,
    path = "/api/v1/scheduler/tasks/{id}",
    tag = "scheduler",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    request_body = UpdateTaskRequest,
    responses(
        (status = 200, description = "Updated task", body = TaskView),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Task not found")
    ),
    security(("api_key" = []))
)]
pub async fn update_task(
    RequireAuth(auth): RequireAuth,
    engine: Option<Extension<Arc<SchedulerEngine>>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateTaskRequest>,
) -> Result<Json<ApiResponse<TaskView>>, crate::middleware::auth::AuthRejection> {
    require_scope(&auth, &cratos_core::Scope::SchedulerWrite)?;
    let Some(Extension(engine)) = engine else {
        return Ok(Json(ApiResponse::error("Scheduler not enabled")));
    };

    // Handle enable/disable
    if let Some(enabled) = request.enabled {
        if let Err(e) = engine.set_task_enabled(id, enabled).await {
            warn!("Failed to set task enabled: {}", e);
        }
    }

    // Fetch current task and return it
    match engine.get_task(id).await {
        Ok(task) => Ok(Json(ApiResponse::success(task_to_view(&task)))),
        Err(e) => Ok(Json(ApiResponse::error(format!("Task not found: {}", e)))),
    }
}

/// Delete a task (requires authentication + scheduler_write scope)
#[utoipa::path(
    delete,
    path = "/api/v1/scheduler/tasks/{id}",
    tag = "scheduler",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Task not found")
    ),
    security(("api_key" = []))
)]
pub async fn delete_task(
    RequireAuth(auth): RequireAuth,
    engine: Option<Extension<Arc<SchedulerEngine>>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, crate::middleware::auth::AuthRejection> {
    require_scope(&auth, &cratos_core::Scope::SchedulerWrite)?;
    let Some(Extension(engine)) = engine else {
        return Ok(Json(ApiResponse::error("Scheduler not enabled")));
    };

    match engine.remove_task(id).await {
        Ok(()) => {
            info!("Deleted task: {}", id);
            Ok(Json(ApiResponse::success(())))
        }
        Err(e) => Ok(Json(ApiResponse::error(format!(
            "Failed to delete task: {}",
            e
        )))),
    }
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

    #[test]
    fn test_valid_trigger_types() {
        assert!(is_valid_trigger_type("cron"));
        assert!(is_valid_trigger_type("interval"));
        assert!(is_valid_trigger_type("one_time"));
        assert!(!is_valid_trigger_type("invalid"));
    }

    #[test]
    fn test_valid_action_types() {
        assert!(is_valid_action_type("notification"));
        assert!(is_valid_action_type("natural_language"));
        assert!(is_valid_action_type("tool_call"));
        assert!(!is_valid_action_type("invalid"));
    }

    #[test]
    fn test_parse_trigger_cron() {
        let config = serde_json::json!({"expression": "0 9 * * *"});
        let trigger = parse_trigger("cron", &config).unwrap();
        assert!(matches!(trigger, TriggerType::Cron(_)));
    }

    #[test]
    fn test_parse_trigger_interval() {
        let config = serde_json::json!({"seconds": 3600});
        let trigger = parse_trigger("interval", &config).unwrap();
        assert!(matches!(trigger, TriggerType::Interval(_)));
    }

    #[test]
    fn test_parse_trigger_invalid() {
        let config = serde_json::json!({});
        let result = parse_trigger("invalid", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_action_natural_language() {
        let config = serde_json::json!({"prompt": "Check status"});
        let action = parse_action("natural_language", &config).unwrap();
        assert!(matches!(action, TaskAction::NaturalLanguage { .. }));
    }

    #[test]
    fn test_parse_action_tool_call() {
        let config = serde_json::json!({"tool": "exec", "args": {"command": "ls"}});
        let action = parse_action("tool_call", &config).unwrap();
        assert!(matches!(action, TaskAction::ToolCall { .. }));
    }

    #[test]
    fn test_parse_action_invalid() {
        let config = serde_json::json!({});
        let result = parse_action("invalid", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_task_to_view() {
        let task = ScheduledTask::new(
            "test_task",
            TriggerType::interval(3600),
            TaskAction::NaturalLanguage {
                prompt: "Hello".to_string(),
                channel: None,
            },
        );
        let view = task_to_view(&task);
        assert_eq!(view.name, "test_task");
        assert_eq!(view.trigger_type, "interval");
        assert_eq!(view.action_type, "natural_language");
        assert!(view.enabled);
    }
}
