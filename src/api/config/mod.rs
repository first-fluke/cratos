//! Configuration API endpoints
//!
//! GET  /api/v1/config - Get current configuration
//! PUT  /api/v1/config - Update configuration

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use cratos_core::auth::Scope;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;

use crate::middleware::auth::{require_scope, RequireAuth};

use crate::server::config::{
    AnthropicLlmConfig, AppConfig, ConfigValidator, GeminiLlmConfig, OpenAiLlmConfig,
};

/// Configuration state shared across API handlers
#[derive(Clone)]
pub struct ConfigState {
    config: Arc<RwLock<AppConfig>>,
}

impl ConfigState {
    /// Create a new config state with defaults
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(AppConfig::default())),
        }
    }

    /// Create from application configuration
    pub fn with_config(config: &AppConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config.clone())),
        }
    }
}

impl Default for ConfigState {
    fn default() -> Self {
        Self::new()
    }
}

impl From<AppConfig> for AppConfigView {
    fn from(config: AppConfig) -> Self {
        // Determine default model based on provider
        let llm_model = match config.llm.default_provider.as_str() {
            "openai" => config
                .llm
                .openai
                .as_ref()
                .map(|c| c.default_model.clone())
                .unwrap_or_else(|| "gpt-5".to_string()),
            "anthropic" => config
                .llm
                .anthropic
                .as_ref()
                .map(|c| c.default_model.clone())
                .unwrap_or_else(|| "claude-sonnet-4-5-20250929".to_string()),
            "gemini" | "google" | "google_pro" => config
                .llm
                .gemini
                .as_ref()
                .map(|c| c.default_model.clone())
                .unwrap_or_else(|| "gemini-2.0-flash".to_string()),
            _ => "auto".to_string(),
        };

        // Build model routing view from config
        let model_routing = config.llm.model_routing.as_ref().map(|mr| {
            ModelRoutingView {
                simple: mr.simple.as_ref().map(|r| RouteEntryView {
                    provider: r.provider.clone(),
                    model: r.model.clone(),
                }),
                general: mr.general.as_ref().map(|r| RouteEntryView {
                    provider: r.provider.clone(),
                    model: r.model.clone(),
                }),
                complex: mr.complex.as_ref().map(|r| RouteEntryView {
                    provider: r.provider.clone(),
                    model: r.model.clone(),
                }),
                fallback: mr.fallback.as_ref().map(|r| RouteEntryView {
                    provider: r.provider.clone(),
                    model: r.model.clone(),
                }),
                auto_downgrade: mr.auto_downgrade.unwrap_or(true),
            }
        });

        Self {
            general: GeneralView {
                language: config.language,
                persona: config.persona,
            },
            llm: LlmView {
                default_provider: config.llm.default_provider,
                model: llm_model,
                model_routing,
            },
            channels: ChannelsView {
                telegram_enabled: config.channels.telegram.enabled,
                slack_enabled: config.channels.slack.enabled,
                discord_enabled: config.channels.discord.enabled,
            },
            security: SecurityView {
                approval_mode: config.approval.default_mode,
                sandbox_policy: config
                    .security
                    .sandbox_policy
                    .unwrap_or_else(|| "moderate".to_string()),
                exec_mode: config.security.exec.mode,
                injection_protection: config
                    .security
                    .enable_injection_protection
                    .unwrap_or(true),
            },
            tools: ToolsView {
                scheduler_enabled: config.scheduler.enabled,
                scheduler_check_interval_secs: config.scheduler.check_interval_secs,
                vector_search_enabled: config.vector_search.enabled,
                browser_enabled: true, // from browser config if available
                mcp_enabled: true,     // from mcp config if available
            },
            advanced: AdvancedView {
                server_port: config.server.port,
                replay_retention_days: config.replay.retention_days,
                redis_url: config.redis.url,
            },
        }
    }
}

// ============================================================================
// AppConfigView — categorized configuration view
// ============================================================================

/// User-facing configuration view (excludes sensitive data), grouped by category
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct AppConfigView {
    pub general: GeneralView,
    pub llm: LlmView,
    pub channels: ChannelsView,
    pub security: SecurityView,
    pub tools: ToolsView,
    pub advanced: AdvancedView,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct GeneralView {
    pub language: String,
    pub persona: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct LlmView {
    pub default_provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_routing: Option<ModelRoutingView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelRoutingView {
    pub simple: Option<RouteEntryView>,
    pub general: Option<RouteEntryView>,
    pub complex: Option<RouteEntryView>,
    pub fallback: Option<RouteEntryView>,
    pub auto_downgrade: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RouteEntryView {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ChannelsView {
    pub telegram_enabled: bool,
    pub slack_enabled: bool,
    pub discord_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct SecurityView {
    pub approval_mode: String,
    pub sandbox_policy: String,
    pub exec_mode: String,
    pub injection_protection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ToolsView {
    pub scheduler_enabled: bool,
    pub scheduler_check_interval_secs: u64,
    pub vector_search_enabled: bool,
    pub browser_enabled: bool,
    pub mcp_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct AdvancedView {
    pub server_port: u16,
    pub replay_retention_days: u32,
    pub redis_url: String,
}

// ============================================================================
// Configuration update request — categorized
// ============================================================================

/// Configuration update request (all fields optional)
#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfigUpdateRequest {
    #[serde(default)]
    pub general: Option<GeneralUpdate>,
    #[serde(default)]
    pub llm: Option<LlmUpdate>,
    #[serde(default)]
    pub channels: Option<ChannelsUpdate>,
    #[serde(default)]
    pub security: Option<SecurityUpdate>,
    #[serde(default)]
    pub tools: Option<ToolsUpdate>,
    #[serde(default)]
    pub advanced: Option<AdvancedUpdate>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GeneralUpdate {
    pub language: Option<String>,
    pub persona: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LlmUpdate {
    pub default_provider: Option<String>,
    pub model: Option<String>,
}

/// Channels update request
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChannelsUpdate {
    pub telegram_enabled: Option<bool>,
    pub slack_enabled: Option<bool>,
    pub discord_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SecurityUpdate {
    pub approval_mode: Option<String>,
    pub exec_mode: Option<String>,
    pub injection_protection: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ToolsUpdate {
    pub scheduler_enabled: Option<bool>,
    pub vector_search_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdvancedUpdate {
    pub server_port: Option<u16>,
    pub replay_retention_days: Option<u32>,
}

// ============================================================================
// Update response
// ============================================================================

/// Configuration update response with reload status
#[derive(Debug, Serialize, ToSchema)]
pub struct ConfigUpdateResponse {
    pub config: AppConfigView,
    pub requires_restart: Vec<String>,
}

/// API response wrapper
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> ApiResponse<T> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Get current configuration (requires ConfigRead scope)
#[utoipa::path(
    get,
    path = "/api/v1/config",
    tag = "config",
    responses(
        (status = 200, description = "Current configuration", body = AppConfigView),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - missing ConfigRead scope")
    ),
    security(("api_key" = []))
)]
pub async fn get_config(
    RequireAuth(auth): RequireAuth,
    State(state): State<ConfigState>,
) -> impl IntoResponse {
    if let Err(rejection) = require_scope(&auth, &Scope::ConfigRead) {
        return rejection.into_response();
    }
    let config = state.config.read().await;
    let view = AppConfigView::from(config.clone());
    Json(ApiResponse::success(view)).into_response()
}

/// Update configuration (requires ConfigWrite scope)
#[utoipa::path(
    put,
    path = "/api/v1/config",
    tag = "config",
    request_body = ConfigUpdateRequest,
    responses(
        (status = 200, description = "Updated configuration", body = ConfigUpdateResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - missing ConfigWrite scope")
    ),
    security(("api_key" = []))
)]
pub async fn update_config(
    RequireAuth(auth): RequireAuth,
    State(state): State<ConfigState>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<ApiResponse<ConfigUpdateResponse>>, StatusCode> {
    auth.require_scope(&Scope::ConfigWrite)
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let mut config = state.config.write().await;
    let mut requires_restart = Vec::new();

    // Apply general updates
    if let Some(general) = request.general {
        if let Some(language) = general.language {
            if let Err(e) = ConfigValidator::validate_language(&language) {
                return Ok(Json(ApiResponse::error(e)));
            }
            config.language = language;
        }
        if let Some(persona) = general.persona {
            if let Err(e) = ConfigValidator::validate_persona(&persona) {
                return Ok(Json(ApiResponse::error(e)));
            }
            config.persona = persona;
        }
    }

    // Apply LLM updates
    if let Some(llm) = request.llm {
        if let Some(provider) = llm.default_provider {
            if let Err(e) = ConfigValidator::validate_provider(&provider) {
                return Ok(Json(ApiResponse::error(e)));
            }
            config.llm.default_provider = provider;
            requires_restart.push("llm.default_provider".to_string());
        }
        if let Some(model) = llm.model {
            match config.llm.default_provider.as_str() {
                "openai" => {
                    let mut c = config.llm.openai.clone().unwrap_or(OpenAiLlmConfig {
                        default_model: "gpt-5".to_string(),
                    });
                    c.default_model = model;
                    config.llm.openai = Some(c);
                }
                "anthropic" => {
                    let mut c = config.llm.anthropic.clone().unwrap_or(AnthropicLlmConfig {
                        default_model: "claude-sonnet-4-5-20250929".to_string(),
                    });
                    c.default_model = model;
                    config.llm.anthropic = Some(c);
                }
                "gemini" | "google" | "google_pro" => {
                    let mut c = config.llm.gemini.clone().unwrap_or(GeminiLlmConfig {
                        default_model: "gemini-2.0-flash".to_string(),
                    });
                    c.default_model = model;
                    config.llm.gemini = Some(c);
                }
                _ => {}
            }
            requires_restart.push("llm.model".to_string());
        }
    }

    // Apply security updates
    if let Some(security) = request.security {
        if let Some(mode) = security.approval_mode {
            if let Err(e) = ConfigValidator::validate_approval_mode(&mode) {
                return Ok(Json(ApiResponse::error(e)));
            }
            config.approval.default_mode = mode;
        }
        if let Some(mode) = security.exec_mode {
            if let Err(e) = ConfigValidator::validate_exec_mode(&mode) {
                return Ok(Json(ApiResponse::error(e)));
            }
            config.security.exec.mode = mode;
        }
        if let Some(v) = security.injection_protection {
            config.security.enable_injection_protection = Some(v);
        }
    }

    // Apply tools updates
    if let Some(tools) = request.tools {
        if let Some(enabled) = tools.scheduler_enabled {
            config.scheduler.enabled = enabled;
        }
        if let Some(enabled) = tools.vector_search_enabled {
            config.vector_search.enabled = enabled;
        }
    }

    // Apply channels updates
    if let Some(channels) = request.channels {
        if let Some(enabled) = channels.telegram_enabled {
            config.channels.telegram.enabled = enabled;
            requires_restart.push("channels.telegram".to_string());
        }
        if let Some(enabled) = channels.slack_enabled {
            config.channels.slack.enabled = enabled;
            requires_restart.push("channels.slack".to_string());
        }
        if let Some(enabled) = channels.discord_enabled {
            config.channels.discord.enabled = enabled;
            requires_restart.push("channels.discord".to_string());
        }
    }

    // Apply advanced updates
    if let Some(advanced) = request.advanced {
        if let Some(port) = advanced.server_port {
            if let Err(e) = ConfigValidator::validate_port(port) {
                return Ok(Json(ApiResponse::error(e)));
            }
            config.server.port = port;
            requires_restart.push("server.port".to_string());
        }
        if let Some(days) = advanced.replay_retention_days {
            config.replay.retention_days = days;
        }
    }

    // Persist changes
    if let Err(e) = config.save("config/local.toml") {
        tracing::error!("Failed to save configuration: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Return the updated view
    let view = AppConfigView::from(config.clone());
    Ok(Json(ApiResponse::success(ConfigUpdateResponse {
        config: view,
        requires_restart,
    })))
}

/// Create configuration routes with explicit state
pub fn config_routes_with_state(state: ConfigState) -> Router {
    Router::new()
        .route("/api/v1/config", get(get_config).put(update_config))
        .with_state(state)
}

#[cfg(test)]
mod tests;
