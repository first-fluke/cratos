//! Configuration API endpoints
//!
//! GET  /api/v1/config - Get current configuration
//! PUT  /api/v1/config - Update configuration

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration state shared across API handlers
#[derive(Clone)]
pub struct ConfigState {
    config: Arc<RwLock<AppConfigView>>,
}

impl ConfigState {
    /// Create a new config state with defaults
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(AppConfigView::default())),
        }
    }

    /// Create from existing config
    #[allow(dead_code)]
    pub fn from_config(config: AppConfigView) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
        }
    }
}

impl Default for ConfigState {
    fn default() -> Self {
        Self::new()
    }
}

/// User-facing configuration view (excludes sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelsView {
    pub telegram_enabled: bool,
    pub slack_enabled: bool,
    pub discord_enabled: bool,
}

/// Configuration update request
#[derive(Debug, Deserialize)]
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
}

/// API response wrapper
#[derive(Debug, Serialize)]
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

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Get current configuration
async fn get_config(State(state): State<ConfigState>) -> Json<ApiResponse<AppConfigView>> {
    let config = state.config.read().await.clone();
    Json(ApiResponse::success(config))
}

/// Update configuration
async fn update_config(
    State(state): State<ConfigState>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<ApiResponse<AppConfigView>>, StatusCode> {
    let mut config = state.config.write().await;

    // Apply updates
    if let Some(provider) = request.llm_provider {
        if !is_valid_provider(&provider) {
            return Ok(Json(ApiResponse::error(format!(
                "Invalid provider: {}",
                provider
            ))));
        }
        config.llm_provider = provider;
    }

    if let Some(model) = request.llm_model {
        config.llm_model = model;
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
        config.approval_mode = mode;
    }

    if let Some(enabled) = request.scheduler_enabled {
        config.scheduler_enabled = enabled;
    }

    Ok(Json(ApiResponse::success(config.clone())))
}

fn is_valid_provider(provider: &str) -> bool {
    matches!(
        provider,
        "openai"
            | "anthropic"
            | "gemini"
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

/// Create configuration routes
pub fn config_routes() -> Router {
    Router::new()
        .route("/api/v1/config", get(get_config).put(update_config))
        .with_state(ConfigState::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_providers() {
        assert!(is_valid_provider("openai"));
        assert!(is_valid_provider("anthropic"));
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
