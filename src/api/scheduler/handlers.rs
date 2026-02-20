use axum::{extract::Path, Extension, Json};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use cratos_core::scheduler::{ScheduledTask, SchedulerEngine};

use super::super::config::ApiResponse;
use super::types::{
    parse_action, parse_trigger, task_to_view, CreateTaskRequest, TaskView, UpdateTaskRequest,
};
use crate::middleware::auth::{require_scope, RequireAuth};

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
    require_scope(&auth, &cratos_core::Scope::SchedulerRead)?;
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
