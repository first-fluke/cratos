use crate::error::{Error, Result};
use crate::util::mask_api_key;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

// ============================================================================
// Constants
// ============================================================================

/// OpenRouter API base URL
pub const BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Popular OpenRouter models (subset)
pub const MODELS: &[&str] = &[
    // Free Models Router (Recommended - auto-selects best free model)
    "openrouter/free",
    // Individual free models
    "qwen/qwen3-32b:free",
    "meta-llama/llama-3.2-3b-instruct:free",
    "google/gemma-2-9b-it:free",
    "stepfun/step-3.5-flash:free",
    "arcee-ai/trinity-large-preview:free",
    "upstage/solar-pro-3:free",
    // OpenAI
    "openai/gpt-4o",
    "openai/gpt-4o-mini",
    // Anthropic
    "anthropic/claude-3.5-sonnet",
    "anthropic/claude-3-haiku",
    // Google
    "google/gemini-pro-1.5",
    "google/gemini-flash-1.5",
    // Meta
    "meta-llama/llama-3.1-405b-instruct",
    "meta-llama/llama-3.1-70b-instruct",
    // Mistral
    "mistralai/mistral-large",
    "mistralai/mixtral-8x22b-instruct",
];

/// Default model (free tier router - auto-selects best free model)
pub const DEFAULT_MODEL: &str = "openrouter/free";

// ============================================================================
// Configuration
// ============================================================================

/// OpenRouter provider configuration
#[derive(Clone)]
pub struct OpenRouterConfig {
    /// API key
    pub api_key: String,
    /// Base URL
    pub base_url: String,
    /// Default model
    pub default_model: String,
    /// Request timeout
    pub timeout: Duration,
    /// App name (for OpenRouter analytics)
    pub app_name: Option<String>,
    /// Site URL (for OpenRouter analytics)
    pub site_url: Option<String>,
}

impl fmt::Debug for OpenRouterConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenRouterConfig")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("timeout", &self.timeout)
            .field("app_name", &self.app_name)
            .finish()
    }
}

impl OpenRouterConfig {
    /// Create a new configuration
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: BASE_URL.to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            timeout: Duration::from_secs(120), // Longer timeout for routing
            app_name: Some("Cratos".to_string()),
            site_url: None,
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| Error::NotConfigured("OPENROUTER_API_KEY not set".to_string()))?;

        let base_url =
            std::env::var("OPENROUTER_BASE_URL").unwrap_or_else(|_| BASE_URL.to_string());
        let default_model =
            std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Ok(Self {
            api_key,
            base_url,
            default_model,
            timeout: Duration::from_secs(120),
            app_name: std::env::var("OPENROUTER_APP_NAME").ok(),
            site_url: std::env::var("OPENROUTER_SITE_URL").ok(),
        })
    }

    /// Set the base URL
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the default model
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Set the timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the app name
    #[must_use]
    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = Some(name.into());
        self
    }

    /// Set the site URL
    #[must_use]
    pub fn with_site_url(mut self, url: impl Into<String>) -> Self {
        self.site_url = Some(url.into());
        self
    }
}

// ============================================================================
// API Types (OpenAI compatible with extensions)
// ============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct OpenRouterRequest {
    pub model: String,
    pub messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenRouterTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    // OpenRouter specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transforms: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OpenRouterMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenRouterToolCall>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OpenRouterTool {
    pub r#type: String,
    pub function: OpenRouterFunction,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OpenRouterFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OpenRouterToolCall {
    pub id: String,
    pub r#type: String,
    pub function: OpenRouterFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OpenRouterFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
pub(crate) struct OpenRouterResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<OpenRouterChoice>,
    pub usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
pub(crate) struct OpenRouterChoice {
    pub index: u32,
    pub message: OpenRouterMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenRouterUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenRouterError {
    pub error: OpenRouterErrorDetail,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
pub(crate) struct OpenRouterErrorDetail {
    pub message: String,
    pub code: Option<i32>,
}
