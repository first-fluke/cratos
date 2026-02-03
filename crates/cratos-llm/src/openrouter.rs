//! OpenRouter - Multi-provider LLM Gateway
//!
//! This module implements the OpenRouter provider which gives access to
//! 100+ models through a single API, including free tier models.
//!
//! Key features:
//! - Single API for multiple providers (OpenAI, Anthropic, Google, Meta, etc.)
//! - Free tier models (Qwen, Llama, etc.)
//! - Automatic fallback and routing
//! - Cost tracking

use crate::error::{Error, Result};
use crate::router::{
    CompletionRequest, CompletionResponse, LlmProvider, Message, MessageRole, TokenUsage, ToolCall,
    ToolChoice, ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tracing::{debug, instrument};

// ============================================================================
// Constants
// ============================================================================

/// OpenRouter API base URL
pub const BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Popular OpenRouter models (subset)
pub const MODELS: &[&str] = &[
    // Free models
    "qwen/qwen3-32b:free",
    "meta-llama/llama-3.2-3b-instruct:free",
    "google/gemma-2-9b-it:free",
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

/// Default model (free tier)
pub const DEFAULT_MODEL: &str = "qwen/qwen3-32b:free";

// ============================================================================
// Security Utilities
// ============================================================================

/// Sanitize API error messages
fn sanitize_api_error(error: &str) -> String {
    let lower = error.to_lowercase();

    if lower.contains("api key")
        || lower.contains("apikey")
        || lower.contains("invalid key")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
    {
        return "API authentication error. Please check your API key configuration.".to_string();
    }

    if lower.contains("rate limit") || lower.contains("quota") {
        return "API rate limit exceeded. Please try again later.".to_string();
    }

    if error.len() < 100 {
        return error.to_string();
    }

    "An API error occurred. Please try again.".to_string()
}

/// Mask API key for safe display
fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

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
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenRouterTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    // OpenRouter specific
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transforms: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenRouterToolCall>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterTool {
    r#type: String,
    function: OpenRouterFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterToolCall {
    id: String,
    r#type: String,
    function: OpenRouterFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenRouterResponse {
    id: String,
    model: String,
    choices: Vec<OpenRouterChoice>,
    usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenRouterChoice {
    index: u32,
    message: OpenRouterMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenRouterError {
    error: OpenRouterErrorDetail,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenRouterErrorDetail {
    message: String,
    code: Option<i32>,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// OpenRouter LLM provider
pub struct OpenRouterProvider {
    client: Client,
    config: OpenRouterConfig,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider
    #[must_use]
    pub fn new(config: OpenRouterConfig) -> Self {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = OpenRouterConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Convert our message to OpenRouter format
    fn convert_message(msg: &Message) -> OpenRouterMessage {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        OpenRouterMessage {
            role: role.to_string(),
            content: msg.content.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            tool_calls: None,
        }
    }

    /// Convert tool definition to OpenRouter format
    fn convert_tool(tool: &ToolDefinition) -> OpenRouterTool {
        OpenRouterTool {
            r#type: "function".to_string(),
            function: OpenRouterFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        }
    }

    /// Convert tool choice to OpenRouter format
    fn convert_tool_choice(choice: &ToolChoice) -> Option<serde_json::Value> {
        match choice {
            ToolChoice::Auto => Some(serde_json::json!("auto")),
            ToolChoice::None => Some(serde_json::json!("none")),
            ToolChoice::Required => Some(serde_json::json!("required")),
            ToolChoice::Tool(name) => Some(serde_json::json!({
                "type": "function",
                "function": {"name": name}
            })),
        }
    }

    /// Make API request
    async fn request<T: serde::de::DeserializeOwned>(&self, body: &OpenRouterRequest) -> Result<T> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let mut request = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json");

        // Add OpenRouter specific headers
        if let Some(app_name) = &self.config.app_name {
            request = request.header("X-Title", app_name);
        }
        if let Some(site_url) = &self.config.site_url {
            request = request.header("HTTP-Referer", site_url);
        }

        let response = request
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !status.is_success() {
            let error: std::result::Result<OpenRouterError, _> = serde_json::from_str(&text);
            let message = error
                .map(|e| e.error.message)
                .unwrap_or_else(|_| text.clone());
            return Err(Error::Api(sanitize_api_error(&message)));
        }

        serde_json::from_str(&text).map_err(|e| Error::InvalidResponse(e.to_string()))
    }

    /// Check if a model is free tier
    #[must_use]
    pub fn is_free_model(model: &str) -> bool {
        model.ends_with(":free")
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    fn supports_tools(&self) -> bool {
        true // Most models support tools
    }

    fn available_models(&self) -> Vec<String> {
        MODELS.iter().map(|s| (*s).to_string()).collect()
    }

    fn default_model(&self) -> &str {
        &self.config.default_model
    }

    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let model = if request.model.is_empty() {
            &self.config.default_model
        } else {
            &request.model
        };

        let messages: Vec<OpenRouterMessage> =
            request.messages.iter().map(Self::convert_message).collect();

        let openrouter_request = OpenRouterRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            tools: None,
            tool_choice: None,
            stop: request.stop.clone(),
            route: None,
            transforms: None,
        };

        debug!("Sending request to OpenRouter API");

        let response: OpenRouterResponse = self.request(&openrouter_request).await?;

        let choice = response
            .choices
            .first()
            .ok_or_else(|| Error::InvalidResponse("No choices in response".to_string()))?;

        let usage = response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(CompletionResponse {
            content: choice.message.content.clone(),
            usage,
            finish_reason: choice.finish_reason.clone(),
            model: response.model,
        })
    }

    #[instrument(skip(self, request), fields(model = %request.request.model, tools = request.tools.len()))]
    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse> {
        let model = if request.request.model.is_empty() {
            &self.config.default_model
        } else {
            &request.request.model
        };

        let messages: Vec<OpenRouterMessage> = request
            .request
            .messages
            .iter()
            .map(Self::convert_message)
            .collect();

        let tools: Vec<OpenRouterTool> = request.tools.iter().map(Self::convert_tool).collect();

        let openrouter_request = OpenRouterRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.request.max_tokens,
            temperature: request.request.temperature,
            tools: Some(tools),
            tool_choice: Self::convert_tool_choice(&request.tool_choice),
            stop: request.request.stop.clone(),
            route: None,
            transforms: None,
        };

        debug!("Sending tool request to OpenRouter API");

        let response: OpenRouterResponse = self.request(&openrouter_request).await?;

        let choice = response
            .choices
            .first()
            .ok_or_else(|| Error::InvalidResponse("No choices in response".to_string()))?;

        let tool_calls: Vec<ToolCall> = choice
            .message
            .tool_calls
            .as_ref()
            .map(|calls| {
                calls
                    .iter()
                    .map(|tc| ToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let usage = response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ToolCompletionResponse {
            content: if choice.message.content.is_empty() {
                None
            } else {
                Some(choice.message.content.clone())
            },
            tool_calls,
            usage,
            finish_reason: choice.finish_reason.clone(),
            model: response.model,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = OpenRouterConfig::new("test-key")
            .with_model("openai/gpt-4o")
            .with_timeout(Duration::from_secs(60))
            .with_app_name("TestApp");

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "openai/gpt-4o");
        assert_eq!(config.app_name, Some("TestApp".to_string()));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"qwen/qwen3-32b:free"));
        assert!(MODELS.contains(&"openai/gpt-4o"));
        assert!(MODELS.contains(&"anthropic/claude-3.5-sonnet"));
    }

    #[test]
    fn test_is_free_model() {
        assert!(OpenRouterProvider::is_free_model("qwen/qwen3-32b:free"));
        assert!(OpenRouterProvider::is_free_model(
            "meta-llama/llama-3.2-3b-instruct:free"
        ));
        assert!(!OpenRouterProvider::is_free_model("openai/gpt-4o"));
    }

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("sk-or-1234567890abcdefghij");
        assert!(masked.starts_with("sk-o"));
        assert!(masked.ends_with("ghij"));
    }

    #[test]
    fn test_convert_message() {
        let msg = Message::assistant("Hello!");
        let converted = OpenRouterProvider::convert_message(&msg);
        assert_eq!(converted.role, "assistant");
        assert_eq!(converted.content, "Hello!");
    }
}
