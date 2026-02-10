//! Web API module for Cratos
//!
//! Provides REST API endpoints for:
//! - Configuration management
//! - Tool listing and information
//! - Execution history
//! - Scheduler management

pub mod browser;
pub mod config;
pub mod executions;
pub mod quota;
pub mod scheduler;
pub mod sessions;
pub mod tools;

use axum::Router;

pub use browser::browser_routes;
pub use config::config_routes;
pub use executions::executions_routes;
pub use quota::quota_routes;
pub use scheduler::scheduler_routes;
pub use sessions::sessions_routes;
pub use tools::tools_routes;

/// Create the API router with all endpoints
pub fn api_router() -> Router {
    Router::new()
        .merge(config_routes())
        .merge(tools_routes())
        .merge(executions_routes())
        .merge(scheduler_routes())
        .merge(quota_routes())
        .merge(sessions_routes())
        .merge(browser_routes())
}
