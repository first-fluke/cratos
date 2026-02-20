//! Executions API endpoints
//!
//! GET /api/v1/executions - List recent executions
//! GET /api/v1/executions/:id - Get execution details

pub mod handlers;
pub mod types;

#[cfg(test)]
mod tests;

pub use handlers::{
    get_execution, get_execution_stats, get_replay_events, list_executions, rerun_execution,
};
pub use types::{EventSummary, ExecutionDetail, ExecutionSummary, ListExecutionsQuery};

use axum::{
    routing::{get, post},
    Router,
};

/// Create executions routes
pub fn executions_routes() -> Router {
    Router::new()
        .route("/api/v1/executions", get(list_executions))
        .route("/api/v1/executions/:id", get(get_execution))
        .route("/api/v1/executions/:id/replay", get(get_replay_events))
        .route("/api/v1/executions/:id/rerun", post(rerun_execution))
        .route("/api/v1/executions/stats", get(get_execution_stats))
}
