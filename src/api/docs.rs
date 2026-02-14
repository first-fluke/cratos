//! API Documentation - Swagger UI
//!
//! Provides OpenAPI documentation at /docs

use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use super::{
    config::{ApiResponse, AppConfigView, ChannelsView, ConfigUpdateRequest},
    executions::{EventSummary, ExecutionDetail, ExecutionSummary, ListExecutionsQuery},
    graph::{GraphData, GraphEdge, GraphNode, GraphQuery, GraphStats},
    pantheon::PersonaSummary,
    quota::{ProviderQuota, QuotaNumbers, QuotaResponse, TodaySummary},
    scheduler::{CreateTaskRequest, TaskView, UpdateTaskRequest},
    skills::SkillInfo,
    tools::ToolInfo,
};

/// Cratos API OpenAPI documentation
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Cratos API",
        version = "1.0.0",
        description = "AI-powered personal assistant REST API.

## Overview
Cratos provides a comprehensive API for:
- **Configuration**: Manage LLM providers, channels, and settings
- **Tools**: List and manage available AI tools
- **Executions**: View and replay execution history
- **Scheduler**: Schedule automated tasks
- **Quota**: Monitor API usage and rate limits
- **Personas**: Manage AI personas (Olympus OS)
- **Graph**: Access knowledge graph data
- **Skills**: Manage auto-generated skills

## Authentication
Most endpoints require authentication via API key in the `Authorization` header:
```
Authorization: Bearer <api_key>
```
",
        contact(
            name = "Cratos Team",
            url = "https://github.com/cratos/cratos"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "/", description = "Local server")
    ),
    paths(
        // Config
        crate::api::config::get_config,
        crate::api::config::update_config,
        // Tools
        crate::api::tools::list_tools,
        // Executions
        crate::api::executions::list_executions,
        crate::api::executions::get_execution,
        crate::api::executions::get_replay_events,
        crate::api::executions::rerun_execution,
        // Scheduler
        crate::api::scheduler::list_tasks,
        crate::api::scheduler::create_task,
        crate::api::scheduler::get_task,
        crate::api::scheduler::update_task,
        crate::api::scheduler::delete_task,
        // Quota
        crate::api::quota::get_quota,
        // Pantheon
        crate::api::pantheon::list_personas,
        crate::api::pantheon::get_persona,
        // Graph
        crate::api::graph::get_graph,
        crate::api::graph::get_graph_stats,
        // Skills
        crate::api::skills::list_skills,
        crate::api::skills::get_skill,
    ),
    components(
        schemas(
            // Config
            ApiResponse<AppConfigView>,
            AppConfigView,
            ChannelsView,
            ConfigUpdateRequest,
            // Tools
            ToolInfo,
            // Executions
            ListExecutionsQuery,
            ExecutionSummary,
            ExecutionDetail,
            EventSummary,
            // Scheduler
            TaskView,
            CreateTaskRequest,
            UpdateTaskRequest,
            // Quota
            QuotaResponse,
            ProviderQuota,
            QuotaNumbers,
            TodaySummary,
            // Pantheon
            PersonaSummary,
            // Graph
            GraphQuery,
            GraphData,
            GraphNode,
            GraphEdge,
            GraphStats,
            // Skills
            SkillInfo,
        )
    ),
    tags(
        (name = "config", description = "Configuration management"),
        (name = "tools", description = "Tool registry operations"),
        (name = "executions", description = "Execution history and replay"),
        (name = "scheduler", description = "Task scheduling"),
        (name = "quota", description = "API usage and rate limits"),
        (name = "pantheon", description = "Persona management (Olympus OS)"),
        (name = "graph", description = "Knowledge graph data"),
        (name = "skills", description = "Auto-generated skill management"),
    )
)]
pub struct ApiDoc;

/// Create documentation routes
pub fn docs_routes() -> Router {
    Router::new().merge(SwaggerUi::new("/docs").url("/api/openapi.json", ApiDoc::openapi()))
}
