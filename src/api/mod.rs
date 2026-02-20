//! Web API module for Cratos
//!
//! Provides REST API endpoints for:
//! - Configuration management
//! - Tool listing and information
//! - Execution history
//! - Scheduler management
//! - Webhooks for external services
//! - API documentation (Swagger UI at /docs)

pub mod auth;
pub mod browser;
pub mod bundle;
pub mod config;
pub mod dev_sessions;
pub mod docs;
pub mod executions;
pub mod graph;
pub mod health;
pub mod nodes;
pub mod pairing;
pub mod pantheon;
pub mod quota;
pub mod scheduler;
pub mod sessions;
pub mod skills;
pub mod tools;
pub mod webhooks;

use axum::Router;

pub use auth::auth_routes;
pub use browser::browser_routes;
pub use bundle::bundle_routes;
pub use config::config_routes_with_state;
pub use dev_sessions::dev_sessions_routes;
pub use docs::docs_routes;
pub use executions::executions_routes;
pub use graph::graph_routes;
pub use health::health_routes;
pub use nodes::nodes_routes;
pub use pairing::pairing_routes;
pub use pantheon::pantheon_routes;
pub use quota::quota_routes;
pub use scheduler::scheduler_routes;
pub use sessions::{sessions_routes_with_state, SessionState};
pub use skills::skills_routes;
pub use tools::tools_routes;
pub use webhooks::webhooks_routes;

use crate::api::config::ConfigState;

/// Create the API router with shared states
pub fn api_router_with_state(session_state: SessionState, config_state: ConfigState) -> Router {
    Router::new()
        .merge(config_routes_with_state(config_state))
        .merge(auth_routes())
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
        .merge(pantheon_routes())
        .merge(graph_routes())
        .merge(nodes_routes())
        .merge(bundle_routes())
}
