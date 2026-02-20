//! Qwen - Alibaba Qwen API Provider
//!
//! This module implements the Qwen provider using Alibaba's DashScope API
//! with OpenAI-compatible endpoint.
//!
//! Supported models:
//! - Qwen-Turbo (fast, cheap)
//! - Qwen-Plus (balanced)
//! - Qwen-Max (most capable)

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

/// DashScope API base URL (OpenAI compatible)
pub const BASE_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";

/// Available Qwen models (2026)
///
/// Qwen 3 family pricing (Alibaba):
/// - qwen3-8b: $0.06/$0.09 per 1M tokens (cheapest)
/// - qwen3-32b: $0.20/$0.30 per 1M tokens (balanced)
/// - qwen-turbo: ~$0.002 per 1K tokens (fast)
/// - qwen-max: ~$0.02 per 1K tokens (best quality)
pub const MODELS: &[&str] = &[
    // Qwen 3 family (2026)
    "qwen3-8b",
    "qwen3-32b",
    "qwen3-235b-a22b", // MoE 235B total, 22B active
    // Qwen 2.5 family (still available)
    "qwen2.5-72b-instruct",
    "qwen2.5-32b-instruct",
    "qwen2.5-7b-instruct",
    // DashScope API models
    "qwen-turbo",
    "qwen-turbo-latest",
    "qwen-plus",
    "qwen-plus-latest",
    "qwen-max",
    "qwen-max-latest",
    "qwen-long",        // Long context
    "qwen-coder-turbo", // Code specialized
    "qwen-coder-plus",
];

/// Default model (Qwen-Turbo - fast and cheap)
pub const DEFAULT_MODEL: &str = "qwen-turbo";

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

/// Qwen provider configuration
#[derive(Clone)]
pub struct QwenConfig {
    /// API key
    pub api_key: String,
    /// Base URL
    pub base_url: String,
    /// Default model
    pub default_model: String,
    /// Request timeout
    pub timeout: Duration,
}

impl fmt::Debug for QwenConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QwenConfig")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl QwenConfig {
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
        let api_key = std::env::var("DASHSCOPE_API_KEY")
            .or_else(|_| std::env::var("QWEN_API_KEY"))
            .map_err(|_| Error::NotConfigured("DASHSCOPE_API_KEY not set".to_string()))?;

        let base_url = std::env::var("DASHSCOPE_BASE_URL").unwrap_or_else(|_| BASE_URL.to_string());
        let default_model =
            std::env::var("QWEN_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

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
struct QwenRequest {
    model: String,
    messages: Vec<QwenMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<QwenTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct QwenMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<QwenToolCall>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct QwenTool {
    r#type: String,
    function: QwenFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct QwenFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct QwenToolCall {
    id: String,
    r#type: String,
    function: QwenFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct QwenFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct QwenResponse {
    id: String,
    model: String,
    choices: Vec<QwenChoice>,
    usage: Option<QwenUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct QwenChoice {
    index: u32,
    message: QwenMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QwenUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct QwenError {
    error: QwenErrorDetail,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct QwenErrorDetail {
    message: String,
    code: Option<String>,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// Qwen LLM provider
pub struct QwenProvider {
    client: Client,
    config: QwenConfig,
}

impl QwenProvider {
    /// Create a new Qwen provider
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(config: QwenConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Provider(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { client, config })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = QwenConfig::from_env()?;
        Self::new(config)
    }

    /// Convert our message to Qwen format
    fn convert_message(msg: &Message) -> QwenMessage {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        QwenMessage {
            role: role.to_string(),
            content: msg.content.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            tool_calls: None,
        }
    }

    /// Convert tool definition to Qwen format
    fn convert_tool(tool: &ToolDefinition) -> QwenTool {
        QwenTool {
            r#type: "function".to_string(),
            function: QwenFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        }
    }

    /// Convert tool choice to Qwen format
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
    async fn request<T: serde::de::DeserializeOwned>(&self, body: &QwenRequest) -> Result<T> {
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
            .update_from_headers("qwen", response.headers())
            .await;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !status.is_success() {
            let error: std::result::Result<QwenError, _> = serde_json::from_str(&text);
            let message = error
                .map(|e| e.error.message)
                .unwrap_or_else(|_| text.clone());
            return Err(Error::Api(sanitize_api_error(&message)));
        }

        serde_json::from_str(&text).map_err(|e| Error::InvalidResponse(e.to_string()))
    }
}

#[async_trait::async_trait]
impl LlmProvider for QwenProvider {
    fn name(&self) -> &str {
        "qwen"
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

        let messages: Vec<QwenMessage> =
            request.messages.iter().map(Self::convert_message).collect();

        let qwen_request = QwenRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            tools: None,
            tool_choice: None,
            stop: request.stop.clone(),
        };

        debug!("Sending request to Qwen API");

        let response: QwenResponse = self.request(&qwen_request).await?;

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

        let messages: Vec<QwenMessage> = request
            .request
            .messages
            .iter()
            .map(Self::convert_message)
            .collect();

        let tools: Vec<QwenTool> = request.tools.iter().map(Self::convert_tool).collect();

        let qwen_request = QwenRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.request.max_tokens,
            temperature: request.request.temperature,
            tools: Some(tools),
            tool_choice: Self::convert_tool_choice(&request.tool_choice),
            stop: request.request.stop.clone(),
        };

        debug!("Sending tool request to Qwen API");

        let response: QwenResponse = self.request(&qwen_request).await?;

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

#[cfg(test)]
mod tests;
