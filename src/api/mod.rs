//! Web API module for Cratos
//!
//! Provides REST API endpoints for:
//! - Configuration management
//! - Tool listing and information
//! - Execution history
//! - Scheduler management

pub mod config;
pub mod executions;
pub mod scheduler;
pub mod tools;

use axum::Router;

pub use config::config_routes;
pub use executions::executions_routes;
pub use scheduler::scheduler_routes;
pub use tools::tools_routes;

/// Create the API router with all endpoints
pub fn api_router() -> Router {
    Router::new()
        .merge(config_routes())
        .merge(tools_routes())
        .merge(executions_routes())
        .merge(scheduler_routes())
}
