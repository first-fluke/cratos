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

use crate::server::config::{AnthropicLlmConfig, AppConfig, GeminiLlmConfig, OpenAiLlmConfig};

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

    /// Create from existing config view (Compatibility - not recommended)
    #[allow(dead_code)]
    pub fn from_config(_config: AppConfigView) -> Self {
        // This is tricky because we can't easily go View -> Config without defaults
        // For now, use default and warn
        Self::new()
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
                .unwrap_or_else(|| "gpt-4o".to_string()),
            "anthropic" => config
                .llm
                .anthropic
                .as_ref()
                .map(|c| c.default_model.clone())
                .unwrap_or_else(|| "claude-3-sonnet-20240229".to_string()),
            "gemini" | "google" | "google_pro" => config
                .llm
                .gemini
                .as_ref()
                .map(|c| c.default_model.clone())
                .unwrap_or_else(|| "gemini-2.0-flash".to_string()),
            _ => "auto".to_string(),
        };

        Self {
            llm_provider: config.llm.default_provider,
            llm_model,
            language: config.language,
            persona: config.persona,
            approval_mode: config.approval.default_mode,
            scheduler_enabled: config.scheduler.enabled,
            vector_search_enabled: config.vector_search.enabled,
            channels: ChannelsView {
                telegram_enabled: config.channels.telegram.enabled,
                slack_enabled: config.channels.slack.enabled,
                discord_enabled: config.channels.discord.enabled,
            },
        }
    }
}

/// User-facing configuration view (excludes sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct AppConfigView {
    /// Current LLM provider
    pub llm_provider: String,
    /// Current LLM model
    pub llm_model: String,
    /// Response language
    pub language: String,
    /// Active persona
    pub persona: String,
    /// Approval mode
    pub approval_mode: String,
    /// Scheduler enabled
    pub scheduler_enabled: bool,
    /// Vector search enabled
    pub vector_search_enabled: bool,
    /// Available channels
    pub channels: ChannelsView,
}

/// Channel configuration view
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ChannelsView {
    pub telegram_enabled: bool,
    pub slack_enabled: bool,
    pub discord_enabled: bool,
}

/// Configuration update request
#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfigUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_search_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<ChannelsUpdate>,
}

/// Channels update request
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChannelsUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telegram_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slack_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_enabled: Option<bool>,
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
        (status = 200, description = "Updated configuration", body = AppConfigView),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - missing ConfigWrite scope")
    ),
    security(("api_key" = []))
)]
pub async fn update_config(
    RequireAuth(auth): RequireAuth,
    State(state): State<ConfigState>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<ApiResponse<AppConfigView>>, StatusCode> {
    auth.require_scope(&Scope::ConfigWrite)
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let mut config = state.config.write().await;

    // Apply updates
    if let Some(provider) = request.llm_provider {
        if !is_valid_provider(&provider) {
            return Ok(Json(ApiResponse::error(format!(
                "Invalid provider: {}",
                provider
            ))));
        }
        config.llm.default_provider = provider;
    }

    // For LLM model, we try to update the default model for the *current* provider if set.
    if let Some(model) = request.llm_model {
        match config.llm.default_provider.as_str() {
            "openai" => {
                let mut c = config.llm.openai.clone().unwrap_or(OpenAiLlmConfig {
                    default_model: "gpt-4o".to_string(),
                });
                c.default_model = model.clone();
                config.llm.openai = Some(c);
            }
            "anthropic" => {
                let mut c = config.llm.anthropic.clone().unwrap_or(AnthropicLlmConfig {
                    default_model: "claude-3-sonnet-20240229".to_string(),
                });
                c.default_model = model.clone();
                config.llm.anthropic = Some(c);
            }
            "gemini" | "google" | "google_pro" => {
                let mut c = config.llm.gemini.clone().unwrap_or(GeminiLlmConfig {
                    default_model: "gemini-2.0-flash".to_string(),
                });
                c.default_model = model.clone();
                config.llm.gemini = Some(c);
            }
            _ => {
                // For other providers or auto, we might not have a specific config struct to update easily here
                // without refactoring LlmConfig.
            }
        }
    }

    if let Some(language) = request.language {
        if !is_valid_language(&language) {
            return Ok(Json(ApiResponse::error(format!(
                "Invalid language: {}",
                language
            ))));
        }
        config.language = language;
    }

    if let Some(persona) = request.persona {
        if !is_valid_persona(&persona) {
            return Ok(Json(ApiResponse::error(format!(
                "Invalid persona: {}",
                persona
            ))));
        }
        config.persona = persona;
    }

    if let Some(mode) = request.approval_mode {
        if !is_valid_approval_mode(&mode) {
            return Ok(Json(ApiResponse::error(format!(
                "Invalid approval mode: {}",
                mode
            ))));
        }
        config.approval.default_mode = mode;
    }

    if let Some(enabled) = request.scheduler_enabled {
        config.scheduler.enabled = enabled;
    }

    if let Some(enabled) = request.vector_search_enabled {
        config.vector_search.enabled = enabled;
    }

    if let Some(channels_update) = request.channels {
        if let Some(enabled) = channels_update.telegram_enabled {
            config.channels.telegram.enabled = enabled;
        }
        if let Some(enabled) = channels_update.slack_enabled {
            config.channels.slack.enabled = enabled;
        }
        if let Some(enabled) = channels_update.discord_enabled {
            config.channels.discord.enabled = enabled;
        }
    }

    // Persist changes
    if let Err(e) = config.save("config/local.json") {
        tracing::error!("Failed to save configuration: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Return the updated view
    let view = AppConfigView::from(config.clone());
    Ok(Json(ApiResponse::success(view)))
}

fn is_valid_provider(provider: &str) -> bool {
    matches!(
        provider,
        "openai"
            | "anthropic"
            | "gemini"
            | "google"
            | "google_pro"
            | "groq"
            | "deepseek"
            | "ollama"
            | "openrouter"
            | "novita"
            | "glm"
            | "moonshot"
            | "qwen"
            | "auto"
    )
}

fn is_valid_language(language: &str) -> bool {
    matches!(language, "en" | "ko" | "ja" | "zh" | "auto")
}

fn is_valid_persona(persona: &str) -> bool {
    matches!(
        persona,
        "cratos" | "athena" | "sindri" | "heimdall" | "mimir"
    )
}

fn is_valid_approval_mode(mode: &str) -> bool {
    matches!(mode, "always" | "risky_only" | "never")
}

/// Create configuration routes with explicit state
pub fn config_routes_with_state(state: ConfigState) -> Router {
    Router::new()
        .route("/api/v1/config", get(get_config).put(update_config))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_providers() {
        assert!(is_valid_provider("openai"));
        assert!(is_valid_provider("anthropic"));
        assert!(is_valid_provider("google_pro"));
        assert!(is_valid_provider("groq"));
        assert!(!is_valid_provider("invalid"));
    }

    #[test]
    fn test_valid_languages() {
        assert!(is_valid_language("en"));
        assert!(is_valid_language("ko"));
        assert!(!is_valid_language("xx"));
    }

    #[test]
    fn test_valid_personas() {
        assert!(is_valid_persona("cratos"));
        assert!(is_valid_persona("sindri"));
        assert!(!is_valid_persona("zeus"));
    }

    #[test]
    fn test_api_response_success() {
        let response: ApiResponse<String> = ApiResponse::success("test".to_string());
        assert!(response.success);
        assert_eq!(response.data, Some("test".to_string()));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<String> = ApiResponse::error("error message");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("error message".to_string()));
    }
}
