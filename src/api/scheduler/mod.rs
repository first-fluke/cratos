//! Scheduler API endpoints
//!
//! GET    /api/v1/scheduler/tasks - List scheduled tasks
//! POST   /api/v1/scheduler/tasks - Create a new task
//! GET    /api/v1/scheduler/tasks/:id - Get task details
//! PUT    /api/v1/scheduler/tasks/:id - Update a task
//! DELETE /api/v1/scheduler/tasks/:id - Delete a task

pub mod handlers;
pub mod types;

#[cfg(test)]
mod tests;

pub use handlers::{create_task, delete_task, get_task, list_tasks, update_task};
pub use types::{CreateTaskRequest, TaskView, UpdateTaskRequest};

use axum::{routing::get, Router};

/// Create scheduler routes
pub fn scheduler_routes() -> Router {
    Router::new()
        .route("/api/v1/scheduler/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/v1/scheduler/tasks/:id",
            get(get_task).put(update_task).delete(delete_task),
        )
}
