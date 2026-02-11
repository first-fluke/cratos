//! Novita AI - Free LLM Provider
//!
//! This module implements the Novita AI provider which offers
//! free access to several models including Llama, Qwen, and GLM.
//!
//! Key features:
//! - Free tier with generous limits
//! - OpenAI-compatible API
//! - Multiple model options

use crate::error::{Error, Result};
use crate::router::{
    CompletionRequest, CompletionResponse, LlmProvider, Message, MessageRole, TokenUsage, ToolCall,
    ToolChoice, ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
};
use crate::util::mask_api_key;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tracing::{debug, instrument};

// ============================================================================
// Constants
// ============================================================================

/// Novita API base URL
pub const BASE_URL: &str = "https://api.novita.ai/v3/openai";

/// Available Novita models (free tier)
pub const MODELS: &[&str] = &[
    // Free models
    "meta-llama/llama-3.2-1b-instruct",
    "meta-llama/llama-3.2-3b-instruct",
    "qwen/qwen2.5-7b-instruct",
    "qwen/qwen2.5-coder-7b-instruct",
    "thudm/glm-4-9b-chat",
    "thudm/glm-z1-9b-chat",
    // Paid models (still cheaper than direct)
    "meta-llama/llama-3.1-70b-instruct",
    "meta-llama/llama-3.1-405b-instruct",
    "qwen/qwen2.5-72b-instruct",
    "deepseek/deepseek-v3",
];

/// Default model (free)
pub const DEFAULT_MODEL: &str = "qwen/qwen2.5-7b-instruct";

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

    // Truncate overly long messages but preserve useful error info
    if error.len() > 300 {
        format!("{}...(truncated)", crate::util::truncate_safe(error, 300))
    } else {
        error.to_string()
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Novita provider configuration
#[derive(Clone)]
pub struct NovitaConfig {
    /// API key
    pub api_key: String,
    /// Base URL
    pub base_url: String,
    /// Default model
    pub default_model: String,
    /// Request timeout
    pub timeout: Duration,
}

impl fmt::Debug for NovitaConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NovitaConfig")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl NovitaConfig {
    /// Create a new configuration
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: BASE_URL.to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            timeout: Duration::from_secs(60),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("NOVITA_API_KEY")
            .map_err(|_| Error::NotConfigured("NOVITA_API_KEY not set".to_string()))?;

        let base_url = std::env::var("NOVITA_BASE_URL").unwrap_or_else(|_| BASE_URL.to_string());
        let default_model =
            std::env::var("NOVITA_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Ok(Self {
            api_key,
            base_url,
            default_model,
            timeout: Duration::from_secs(60),
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
}

// ============================================================================
// API Types (OpenAI compatible)
// ============================================================================

#[derive(Debug, Serialize)]
struct NovitaRequest {
    model: String,
    messages: Vec<NovitaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<NovitaTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NovitaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<NovitaToolCall>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NovitaTool {
    r#type: String,
    function: NovitaFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct NovitaFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct NovitaToolCall {
    id: String,
    r#type: String,
    function: NovitaFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct NovitaFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct NovitaResponse {
    id: String,
    model: String,
    choices: Vec<NovitaChoice>,
    usage: Option<NovitaUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct NovitaChoice {
    index: u32,
    message: NovitaMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NovitaUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct NovitaError {
    error: NovitaErrorDetail,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct NovitaErrorDetail {
    message: String,
    code: Option<String>,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// Novita AI LLM provider
pub struct NovitaProvider {
    client: Client,
    config: NovitaConfig,
}

impl NovitaProvider {
    /// Create a new Novita provider
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(config: NovitaConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Provider(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { client, config })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = NovitaConfig::from_env()?;
        Self::new(config)
    }

    /// Convert our message to Novita format
    fn convert_message(msg: &Message) -> NovitaMessage {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        NovitaMessage {
            role: role.to_string(),
            content: msg.content.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            tool_calls: None,
        }
    }

    /// Convert tool definition to Novita format
    fn convert_tool(tool: &ToolDefinition) -> NovitaTool {
        NovitaTool {
            r#type: "function".to_string(),
            function: NovitaFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        }
    }

    /// Convert tool choice to Novita format
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
    async fn request<T: serde::de::DeserializeOwned>(&self, body: &NovitaRequest) -> Result<T> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        // Capture rate limit headers before consuming the body
        crate::quota::global_quota_tracker()
            .update_from_headers("novita", response.headers())
            .await;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !status.is_success() {
            let error: std::result::Result<NovitaError, _> = serde_json::from_str(&text);
            let message = error
                .map(|e| e.error.message)
                .unwrap_or_else(|_| text.clone());
            return Err(Error::Api(sanitize_api_error(&message)));
        }

        serde_json::from_str(&text).map_err(|e| Error::InvalidResponse(e.to_string()))
    }

    /// Check if a model is in the free tier
    #[must_use]
    pub fn is_free_model(model: &str) -> bool {
        let free_models = [
            "meta-llama/llama-3.2-1b-instruct",
            "meta-llama/llama-3.2-3b-instruct",
            "qwen/qwen2.5-7b-instruct",
            "qwen/qwen2.5-coder-7b-instruct",
            "thudm/glm-4-9b-chat",
            "thudm/glm-z1-9b-chat",
        ];
        free_models.contains(&model)
    }
}

#[async_trait::async_trait]
impl LlmProvider for NovitaProvider {
    fn name(&self) -> &str {
        "novita"
    }

    fn supports_tools(&self) -> bool {
        true
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

        let messages: Vec<NovitaMessage> =
            request.messages.iter().map(Self::convert_message).collect();

        let novita_request = NovitaRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            tools: None,
            tool_choice: None,
            stop: request.stop.clone(),
        };

        debug!("Sending request to Novita API");

        let response: NovitaResponse = self.request(&novita_request).await?;

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

        let messages: Vec<NovitaMessage> = request
            .request
            .messages
            .iter()
            .map(Self::convert_message)
            .collect();

        let tools: Vec<NovitaTool> = request.tools.iter().map(Self::convert_tool).collect();

        let novita_request = NovitaRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.request.max_tokens,
            temperature: request.request.temperature,
            tools: Some(tools),
            tool_choice: Self::convert_tool_choice(&request.tool_choice),
            stop: request.request.stop.clone(),
        };

        debug!("Sending tool request to Novita API");

        let response: NovitaResponse = self.request(&novita_request).await?;

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
                        thought_signature: None,
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
    use crate::util::mask_api_key;

    #[test]
    fn test_config_builder() {
        let config = NovitaConfig::new("test-key")
            .with_model("thudm/glm-4-9b-chat")
            .with_timeout(Duration::from_secs(30));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "thudm/glm-4-9b-chat");
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"qwen/qwen2.5-7b-instruct"));
        assert!(MODELS.contains(&"thudm/glm-4-9b-chat"));
        assert!(MODELS.contains(&"meta-llama/llama-3.2-3b-instruct"));
    }

    #[test]
    fn test_is_free_model() {
        assert!(NovitaProvider::is_free_model("qwen/qwen2.5-7b-instruct"));
        assert!(NovitaProvider::is_free_model("thudm/glm-4-9b-chat"));
        assert!(!NovitaProvider::is_free_model(
            "meta-llama/llama-3.1-405b-instruct"
        ));
    }

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("novita-1234567890abcdef");
        assert!(masked.starts_with("novi"));
        assert!(masked.ends_with("cdef"));
    }

    #[test]
    fn test_convert_message() {
        let msg = Message::tool_response("call_123", "result");
        let converted = NovitaProvider::convert_message(&msg);
        assert_eq!(converted.role, "tool");
        assert_eq!(converted.tool_call_id, Some("call_123".to_string()));
    }
}
