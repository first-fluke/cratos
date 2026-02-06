//! SiliconFlow - Ultra-low-cost LLM provider
//!
//! SiliconFlow provides the cheapest models available (2026):
//! - DeepSeek R1 Distill 70B: $0.03/$0.09 per 1M tokens
//! - Qwen 2.5-VL 7B: $0.05/$0.05 per 1M tokens (vision)
//! - Llama 3.1 8B: $0.06/$0.06 per 1M tokens
//! - GLM-4-9B: $0.086/$0.086 per 1M tokens
//!
//! Uses OpenAI-compatible API.

use crate::error::{Error, Result};
use crate::router::{
    CompletionRequest, CompletionResponse, LlmProvider, Message, TokenUsage, ToolCall, ToolChoice,
    ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
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

/// SiliconFlow API base URL
pub const SILICONFLOW_API_BASE: &str = "https://api.siliconflow.cn/v1";

/// Available SiliconFlow models (2026, sorted by cost)
///
/// Pricing (per 1M tokens):
/// - DeepSeek R1 Distill 70B: $0.03 input / $0.09 output (cheapest!)
/// - Qwen 2.5-VL 7B: $0.05 / $0.05 (vision supported)
/// - Llama 3.1 8B: $0.06 / $0.06
/// - GLM-4-9B: $0.086 / $0.086
/// - Llama 3.1 70B: $0.35 / $0.35
pub const MODELS: &[&str] = &[
    // Ultra-low-cost models
    "deepseek-ai/DeepSeek-R1-Distill-Llama-70B",
    "deepseek-ai/DeepSeek-R1-Distill-Qwen-32B",
    // Budget models
    "Qwen/Qwen2.5-VL-7B-Instruct",
    "meta-llama/Llama-3.1-8B-Instruct",
    "THUDM/GLM-4-9B-0414",
    // Standard models
    "meta-llama/Llama-3.1-70B-Instruct",
    "Qwen/Qwen2.5-72B-Instruct",
];

/// Default SiliconFlow model (cheapest)
pub const DEFAULT_MODEL: &str = "deepseek-ai/DeepSeek-R1-Distill-Llama-70B";

// ============================================================================
// Security Utilities
// ============================================================================

/// Sanitize API error messages to prevent leaking sensitive information
fn sanitize_api_error(error: &str) -> String {
    let lower = error.to_lowercase();

    if lower.contains("api key")
        || lower.contains("apikey")
        || lower.contains("invalid key")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
    {
        return "API authentication error. Please check your SILICONFLOW_API_KEY.".to_string();
    }

    if lower.contains("rate limit") || lower.contains("quota") {
        return "SiliconFlow rate limit exceeded. Please try again later.".to_string();
    }

    if lower.contains("internal") || lower.contains("server error") {
        return "SiliconFlow server error. Please try again later.".to_string();
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

/// SiliconFlow provider configuration
#[derive(Clone)]
pub struct SiliconFlowConfig {
    /// API key
    pub api_key: String,
    /// Base URL
    pub base_url: String,
    /// Default model
    pub default_model: String,
    /// Request timeout
    pub timeout: Duration,
}

// SECURITY: Custom Debug implementation to mask API key
impl fmt::Debug for SiliconFlowConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SiliconFlowConfig")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl SiliconFlowConfig {
    /// Create a new configuration with an API key
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: SILICONFLOW_API_BASE.to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            timeout: Duration::from_secs(120),
        }
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("SILICONFLOW_API_KEY")
            .map_err(|_| Error::NotConfigured("SILICONFLOW_API_KEY not set".to_string()))?;

        let default_model =
            std::env::var("SILICONFLOW_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Ok(Self {
            api_key,
            base_url: SILICONFLOW_API_BASE.to_string(),
            default_model,
            timeout: Duration::from_secs(120),
        })
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
// API Types (OpenAI Compatible)
// ============================================================================

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ChatTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Serialize)]
struct ChatTool {
    r#type: String,
    function: ChatFunction,
}

#[derive(Serialize)]
struct ChatFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
    model: String,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(Deserialize)]
struct ChatToolCall {
    id: String,
    function: ChatToolCallFunction,
}

#[derive(Deserialize)]
struct ChatToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// SiliconFlow provider (OpenAI-compatible)
pub struct SiliconFlowProvider {
    client: Client,
    config: SiliconFlowConfig,
}

impl SiliconFlowProvider {
    /// Create a new SiliconFlow provider
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(config: SiliconFlowConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Provider(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { client, config })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = SiliconFlowConfig::from_env()?;
        Self::new(config)
    }

    fn convert_message(msg: &Message) -> ChatMessage {
        ChatMessage {
            role: msg.role.as_str().to_string(),
            content: msg.content.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            name: msg.name.clone(),
        }
    }

    fn convert_tool(tool: &ToolDefinition) -> ChatTool {
        ChatTool {
            r#type: "function".to_string(),
            function: ChatFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        }
    }

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
}

#[async_trait::async_trait]
impl LlmProvider for SiliconFlowProvider {
    fn name(&self) -> &str {
        "siliconflow"
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

        let messages: Vec<ChatMessage> =
            request.messages.iter().map(Self::convert_message).collect();

        let chat_request = ChatRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stop: request.stop.clone(),
            tools: None,
            tool_choice: None,
        };

        debug!("Sending request to SiliconFlow");

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&chat_request)
            .send()
            .await
            .map_err(|e| Error::Api(sanitize_api_error(&e.to_string())))?;

        // Capture rate limit headers before consuming the body
        crate::quota::global_quota_tracker()
            .update_from_headers("siliconflow", response.headers())
            .await;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::Api(sanitize_api_error(&error_text)));
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| Error::InvalidResponse(e.to_string()))?;

        let choice = chat_response
            .choices
            .first()
            .ok_or_else(|| Error::InvalidResponse("No choices in response".to_string()))?;

        let content = choice.message.content.clone().unwrap_or_default();

        let usage = chat_response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(CompletionResponse {
            content,
            usage,
            finish_reason: choice.finish_reason.clone(),
            model: chat_response.model,
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

        let messages: Vec<ChatMessage> = request
            .request
            .messages
            .iter()
            .map(Self::convert_message)
            .collect();

        let tools: Vec<ChatTool> = request.tools.iter().map(Self::convert_tool).collect();

        let chat_request = ChatRequest {
            model: model.to_string(),
            messages,
            max_tokens: request.request.max_tokens,
            temperature: request.request.temperature,
            stop: request.request.stop.clone(),
            tools: Some(tools),
            tool_choice: Self::convert_tool_choice(&request.tool_choice),
        };

        debug!("Sending tool request to SiliconFlow");

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&chat_request)
            .send()
            .await
            .map_err(|e| Error::Api(sanitize_api_error(&e.to_string())))?;

        // Capture rate limit headers before consuming the body
        crate::quota::global_quota_tracker()
            .update_from_headers("siliconflow", response.headers())
            .await;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::Api(sanitize_api_error(&error_text)));
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| Error::InvalidResponse(e.to_string()))?;

        let choice = chat_response
            .choices
            .first()
            .ok_or_else(|| Error::InvalidResponse("No choices in response".to_string()))?;

        let content = choice.message.content.clone();

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

        let usage = chat_response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ToolCompletionResponse {
            content,
            tool_calls,
            usage,
            finish_reason: choice.finish_reason.clone(),
            model: chat_response.model,
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
        let config = SiliconFlowConfig::new("test-key")
            .with_model("Qwen/Qwen2.5-VL-7B-Instruct")
            .with_timeout(Duration::from_secs(90));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "Qwen/Qwen2.5-VL-7B-Instruct");
        assert_eq!(config.timeout, Duration::from_secs(90));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"deepseek-ai/DeepSeek-R1-Distill-Llama-70B"));
        assert!(MODELS.contains(&"Qwen/Qwen2.5-VL-7B-Instruct"));
        assert!(MODELS.contains(&"meta-llama/Llama-3.1-8B-Instruct"));
    }

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("sk-1234567890abcdefghijklmnop");
        assert!(masked.starts_with("sk-1"));
        assert!(masked.ends_with("mnop"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_sanitize_api_error() {
        let sanitized = sanitize_api_error("Invalid API key: sk-1234567890");
        assert!(!sanitized.contains("sk-"));
        assert!(sanitized.contains("SILICONFLOW_API_KEY"));

        let sanitized = sanitize_api_error("Rate limit exceeded");
        assert!(sanitized.contains("rate limit"));
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = SiliconFlowConfig::new("sk-1234567890abcdefghijklmnop");
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.contains("1234567890abcdefghijkl"));
    }

    #[test]
    fn test_default_model_is_cheapest() {
        // Default should be the cheapest model
        assert_eq!(DEFAULT_MODEL, "deepseek-ai/DeepSeek-R1-Distill-Llama-70B");
    }
}
