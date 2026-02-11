//! GLM - ZhipuAI GLM API Provider
//!
//! This module implements the GLM (General Language Model) provider
//! for ZhipuAI's BigModel platform.
//!
//! Supported models:
//! - GLM-4-9B
//! - GLM-4.5
//! - GLM-Z1-9B (Thinking model)

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

/// GLM API base URL
pub const BASE_URL: &str = "https://open.bigmodel.cn/api/paas/v4";

/// Available GLM models (2026)
///
/// GLM pricing (ZhipuAI):
/// - glm-4-9b: $0.086 per 1M tokens (cheapest, local deployable)
/// - glm-4-flash: $0.01/$0.01 per 1M tokens (fast)
/// - glm-4-plus: $0.14/$0.14 per 1M tokens (balanced)
pub const MODELS: &[&str] = &[
    // GLM-4 family (2026)
    "glm-4-plus",
    "glm-4-flash",
    "glm-4-9b",
    // GLM-4.7 (latest, 2026)
    "glm-4.7",
    "glm-4.7-flash",
    // GLM-Z1 (reasoning)
    "glm-z1-9b",
];

/// Default model (GLM-4 Flash - fast and cheap)
pub const DEFAULT_MODEL: &str = "glm-4-flash";

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

/// GLM provider configuration
#[derive(Clone)]
pub struct GlmConfig {
    /// API key
    pub api_key: String,
    /// Base URL (optional override)
    pub base_url: String,
    /// Default model
    pub default_model: String,
    /// Request timeout
    pub timeout: Duration,
}

impl fmt::Debug for GlmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlmConfig")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl GlmConfig {
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
        let api_key = std::env::var("ZHIPU_API_KEY")
            .or_else(|_| std::env::var("GLM_API_KEY"))
            .map_err(|_| Error::NotConfigured("ZHIPU_API_KEY not set".to_string()))?;

        let base_url = std::env::var("BIGMODEL_BASE_URL").unwrap_or_else(|_| BASE_URL.to_string());
        let default_model =
            std::env::var("GLM_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

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
// API Types
// ============================================================================

#[derive(Debug, Serialize)]
struct GlmRequest {
    model: String,
    messages: Vec<GlmMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GlmTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GlmMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<GlmToolCall>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GlmTool {
    r#type: String,
    function: GlmFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct GlmFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct GlmToolCall {
    id: String,
    r#type: String,
    function: GlmFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct GlmFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct GlmResponse {
    id: String,
    model: String,
    choices: Vec<GlmChoice>,
    usage: Option<GlmUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct GlmChoice {
    index: u32,
    message: GlmMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GlmUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct GlmError {
    error: GlmErrorDetail,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct GlmErrorDetail {
    message: String,
    code: Option<String>,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// GLM LLM provider
pub struct GlmProvider {
    client: Client,
    config: GlmConfig,
}

impl GlmProvider {
    /// Create a new GLM provider
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(config: GlmConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Provider(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { client, config })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = GlmConfig::from_env()?;
        Self::new(config)
    }

    /// Convert our message to GLM format
    fn convert_message(msg: &Message) -> GlmMessage {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        GlmMessage {
            role: role.to_string(),
            content: msg.content.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            tool_calls: None,
        }
    }

    /// Convert tool definition to GLM format
    fn convert_tool(tool: &ToolDefinition) -> GlmTool {
        GlmTool {
            r#type: "function".to_string(),
            function: GlmFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        }
    }

    /// Convert tool choice to GLM format
    fn convert_tool_choice(choice: &ToolChoice) -> Option<String> {
        match choice {
            ToolChoice::Auto => Some("auto".to_string()),
            ToolChoice::None => Some("none".to_string()),
            ToolChoice::Required => Some("required".to_string()),
            ToolChoice::Tool(name) => Some(name.clone()),
        }
    }

    /// Make API request
    async fn request<T: serde::de::DeserializeOwned>(&self, body: &GlmRequest) -> Result<T> {
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
            .update_from_headers("glm", response.headers())
            .await;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !status.is_success() {
            let error: std::result::Result<GlmError, _> = serde_json::from_str(&text);
            let message = error
                .map(|e| e.error.message)
                .unwrap_or_else(|_| text.clone());
            return Err(Error::Api(sanitize_api_error(&message)));
        }

        serde_json::from_str(&text).map_err(|e| Error::InvalidResponse(e.to_string()))
    }
}

#[async_trait::async_trait]
impl LlmProvider for GlmProvider {
    fn name(&self) -> &str {
        "glm"
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

        let messages: Vec<GlmMessage> =
            request.messages.iter().map(Self::convert_message).collect();

        let glm_request = GlmRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            tools: None,
            tool_choice: None,
            stop: request.stop.clone(),
        };

        debug!("Sending request to GLM API");

        let response: GlmResponse = self.request(&glm_request).await?;

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

        let messages: Vec<GlmMessage> = request
            .request
            .messages
            .iter()
            .map(Self::convert_message)
            .collect();

        let tools: Vec<GlmTool> = request.tools.iter().map(Self::convert_tool).collect();

        let glm_request = GlmRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.request.max_tokens,
            temperature: request.request.temperature,
            tools: Some(tools),
            tool_choice: Self::convert_tool_choice(&request.tool_choice),
            stop: request.request.stop.clone(),
        };

        debug!("Sending tool request to GLM API");

        let response: GlmResponse = self.request(&glm_request).await?;

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
        let config = GlmConfig::new("test-key")
            .with_model("glm-4-plus")
            .with_timeout(Duration::from_secs(30));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "glm-4-plus");
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"glm-4.7"));
        assert!(MODELS.contains(&"glm-4.7-flash"));
    }

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("1234567890abcdefghij");
        assert!(masked.starts_with("1234"));
        assert!(masked.ends_with("ghij"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = GlmConfig::new("1234567890abcdefghij");
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.contains("567890abcdef"));
    }

    #[test]
    fn test_convert_message() {
        let msg = Message::user("Hello");
        let converted = GlmProvider::convert_message(&msg);
        assert_eq!(converted.role, "user");
        assert_eq!(converted.content, "Hello");
    }

    #[test]
    fn test_convert_tool_choice() {
        assert_eq!(
            GlmProvider::convert_tool_choice(&ToolChoice::Auto),
            Some("auto".to_string())
        );
        assert_eq!(
            GlmProvider::convert_tool_choice(&ToolChoice::None),
            Some("none".to_string())
        );
    }
}
