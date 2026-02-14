//! Web API module for Cratos
//!
//! Provides REST API endpoints for:
//! - Configuration management
//! - Tool listing and information
//! - Execution history
//! - Scheduler management
//! - Webhooks for external services

pub mod browser;
pub mod config;
pub mod dev_sessions;
pub mod executions;
pub mod health;
pub mod pairing;
pub mod quota;
pub mod scheduler;
pub mod sessions;
pub mod skills;
pub mod tools;
pub mod webhooks;

use axum::Router;

pub use browser::browser_routes;
pub use config::config_routes;
pub use dev_sessions::dev_sessions_routes;
pub use executions::executions_routes;
pub use health::health_routes;
pub use pairing::pairing_routes;
pub use quota::quota_routes;
pub use scheduler::scheduler_routes;
pub use sessions::{sessions_routes_with_state, SessionState};
pub use skills::skills_routes;
pub use tools::tools_routes;
pub use webhooks::webhooks_routes;

/// Create the API router with all endpoints (default SessionState)
#[allow(dead_code)]
pub fn api_router() -> Router {
    api_router_with_session_state(SessionState::new())
}

/// Create the API router with a shared SessionState (for E2E cipher sharing with WS)
pub fn api_router_with_session_state(session_state: SessionState) -> Router {
    Router::new()
        .merge(config_routes())
        .merge(tools_routes())
        .merge(executions_routes())
        .merge(scheduler_routes())
        .merge(quota_routes())
        .merge(sessions_routes_with_state(session_state))
        .merge(browser_routes())
        .merge(dev_sessions_routes())
        .merge(pairing_routes())
        .merge(webhooks_routes())
        .merge(skills_routes())
}
