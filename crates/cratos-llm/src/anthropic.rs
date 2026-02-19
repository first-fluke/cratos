//! Anthropic - Claude API provider
//!
//! This module implements the Anthropic Claude provider using reqwest.

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
// Security Utilities
// ============================================================================

/// Sanitize Anthropic API error messages to prevent leaking sensitive information
fn sanitize_api_error(error: &str) -> String {
    let lower = error.to_lowercase();

    // Don't expose authentication details
    if lower.contains("api key")
        || lower.contains("apikey")
        || lower.contains("invalid key")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
        || lower.contains("x-api-key")
    {
        return "API authentication error. Please check your API key configuration.".to_string();
    }

    // Don't expose rate limit details
    if lower.contains("rate limit") || lower.contains("quota") || lower.contains("overloaded") {
        return "API rate limit exceeded. Please try again later.".to_string();
    }

    // Don't expose internal server errors
    if lower.contains("internal") || lower.contains("server error") {
        return "API server error. Please try again later.".to_string();
    }

    // Truncate overly long messages but preserve useful error info
    if error.len() > 300 {
        format!("{}...(truncated)", crate::util::truncate_safe(error, 300))
    } else {
        error.to_string()
    }
}

/// Anthropic API version
const API_VERSION: &str = "2023-06-01";

/// Available Anthropic models (2026)
///
/// Claude 4.5 family pricing (per 1M tokens):
/// - claude-haiku-4-5: $1.00/$5.00 (fastest, cheapest 4.5)
/// - claude-sonnet-4-5: $3.00/$15.00 (balanced)
/// - claude-opus-4-5: $5.00/$25.00 (most capable, 67% cheaper than opus 4!)
///
/// Claude 4 family (legacy, still available):
/// - claude-sonnet-4: $3.00/$15.00
/// - claude-opus-4: $15.00/$75.00
pub const MODELS: &[&str] = &[
    // Claude 4.5 family (latest)
    "claude-opus-4-5-20250514",
    "claude-sonnet-4-5-20250929",
    "claude-haiku-4-5-20251001",
    // Claude 4 family (legacy)
    "claude-opus-4-20250514",
    "claude-sonnet-4-20250514",
    // Claude 3.5 family (legacy)
    "claude-3-5-sonnet-20241022",
    "claude-3-5-haiku-20241022",
];

/// Default — Claude Sonnet 4.5 (same price as Sonnet 4, but newer/better)
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";

/// Default API base URL
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

// ============================================================================
// API Types
// ============================================================================

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<AnthropicToolChoice>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum AnthropicToolChoice {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "any")]
    Any,
    #[serde(rename = "tool")]
    Tool { name: String },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct AnthropicResponse {
    id: String,
    model: String,
    content: Vec<ResponseContentBlock>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ResponseContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AnthropicError {
    error: AnthropicErrorDetail,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorDetail {
    r#type: String,
    message: String,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// Anthropic provider configuration
#[derive(Clone)]
pub struct AnthropicConfig {
    /// API key
    pub api_key: String,
    /// Base URL
    pub base_url: String,
    /// Default model
    pub default_model: String,
    /// Default max tokens
    pub default_max_tokens: u32,
    /// Request timeout
    pub timeout: Duration,
}

// SECURITY: Custom Debug implementation to mask API key
impl fmt::Debug for AnthropicConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnthropicConfig")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("default_max_tokens", &self.default_max_tokens)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl AnthropicConfig {
    /// Create a new configuration with an API key
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            default_max_tokens: 4096,
            timeout: Duration::from_secs(60),
        }
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| Error::NotConfigured("ANTHROPIC_API_KEY not set".to_string()))?;

        let base_url =
            std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let default_model =
            std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Ok(Self {
            api_key,
            base_url,
            default_model,
            default_max_tokens: 4096,
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

    /// Set the default max tokens
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.default_max_tokens = max_tokens;
        self
    }

    /// Set the timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Anthropic Claude provider
pub struct AnthropicProvider {
    client: Client,
    config: AnthropicConfig,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(config: AnthropicConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;

        Ok(Self { client, config })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = AnthropicConfig::from_env()?;
        Self::new(config)
    }

    /// Convert our message to Anthropic format, returning system message separately
    fn convert_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system_parts = Vec::new();
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // Accumulate system messages
                    if !msg.content.is_empty() {
                        system_parts.push(msg.content.clone());
                    }
                }
                MessageRole::User => {
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Text(msg.content.clone()),
                    });
                }
                MessageRole::Assistant => {
                    anthropic_messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: AnthropicContent::Text(msg.content.clone()),
                    });
                }
                MessageRole::Tool => {
                    if let Some(tool_call_id) = &msg.tool_call_id {
                        anthropic_messages.push(AnthropicMessage {
                            role: "user".to_string(),
                            content: AnthropicContent::Blocks(vec![ContentBlock::ToolResult {
                                tool_use_id: tool_call_id.clone(),
                                content: msg.content.clone(),
                                is_error: None,
                            }]),
                        });
                    }
                }
            }
        }

        let system_message = if !system_parts.is_empty() {
            Some(system_parts.join("\n\n"))
        } else {
            None
        };

        (system_message, anthropic_messages)
    }

    /// Convert tool definition to Anthropic format
    fn convert_tool(tool: &ToolDefinition) -> AnthropicTool {
        AnthropicTool {
            name: tool.name.clone(),
            description: tool.description.clone(),
            input_schema: tool.parameters.clone(),
        }
    }

    /// Convert tool choice to Anthropic format
    fn convert_tool_choice(choice: &ToolChoice) -> Option<AnthropicToolChoice> {
        match choice {
            ToolChoice::Auto => Some(AnthropicToolChoice::Auto),
            ToolChoice::None => None, // Anthropic doesn't have a "none" option, we just don't send tools
            ToolChoice::Required => Some(AnthropicToolChoice::Any),
            ToolChoice::Tool(name) => Some(AnthropicToolChoice::Tool { name: name.clone() }),
        }
    }

    /// Send request to Anthropic API
    async fn send_request(&self, request: AnthropicRequest) -> Result<AnthropicResponse> {
        let url = format!("{}/v1/messages", self.config.base_url);

        debug!("Sending request to Anthropic: {}", url);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        // Capture rate limit headers before consuming the body
        crate::quota::global_quota_tracker()
            .update_from_headers("anthropic", response.headers())
            .await;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !status.is_success() {
            // Try to parse error response
            if let Ok(error) = serde_json::from_str::<AnthropicError>(&body) {
                if status.as_u16() == 429 {
                    return Err(Error::RateLimit);
                }
                // SECURITY: Sanitize error messages
                return Err(Error::Api(sanitize_api_error(&format!(
                    "{}: {}",
                    error.error.r#type, error.error.message
                ))));
            }
            // SECURITY: Don't expose raw HTTP response body
            return Err(Error::Api(sanitize_api_error(&format!(
                "HTTP {}: {}",
                status, body
            ))));
        }

        serde_json::from_str(&body).map_err(|e| Error::InvalidResponse(e.to_string()))
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
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

        let (system, messages) = Self::convert_messages(&request.messages);

        let anthropic_request = AnthropicRequest {
            model: model.to_string(),
            max_tokens: request.max_tokens.unwrap_or(self.config.default_max_tokens),
            system,
            messages,
            temperature: request.temperature,
            tools: None,
            tool_choice: None,
        };

        let response = self.send_request(anthropic_request).await?;

        // Extract text content
        let content = response
            .content
            .iter()
            .filter_map(|block| match block {
                ResponseContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let usage = TokenUsage {
            prompt_tokens: response.usage.input_tokens,
            completion_tokens: response.usage.output_tokens,
            total_tokens: response.usage.input_tokens + response.usage.output_tokens,
        };

        Ok(CompletionResponse {
            content,
            usage: Some(usage),
            finish_reason: response.stop_reason,
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

        let (system, messages) = Self::convert_messages(&request.request.messages);

        let tools: Vec<AnthropicTool> = request.tools.iter().map(Self::convert_tool).collect();

        let anthropic_request = AnthropicRequest {
            model: model.to_string(),
            max_tokens: request
                .request
                .max_tokens
                .unwrap_or(self.config.default_max_tokens),
            system,
            messages,
            temperature: request.request.temperature,
            tools: Some(tools),
            tool_choice: Self::convert_tool_choice(&request.tool_choice),
        };

        let response = self.send_request(anthropic_request).await?;

        // Extract text content and tool calls
        let mut content = None;
        let mut tool_calls = Vec::new();

        for block in &response.content {
            match block {
                ResponseContentBlock::Text { text } => {
                    content = Some(text.clone());
                }
                ResponseContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: serde_json::to_string(input)
                            .unwrap_or_else(|_| "{}".to_string()),
                        thought_signature: None,
                    });
                }
            }
        }

        let usage = TokenUsage {
            prompt_tokens: response.usage.input_tokens,
            completion_tokens: response.usage.output_tokens,
            total_tokens: response.usage.input_tokens + response.usage.output_tokens,
        };

        Ok(ToolCompletionResponse {
            content,
            tool_calls,
            usage: Some(usage),
            finish_reason: response.stop_reason,
            model: response.model,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = AnthropicConfig::new("test-key")
            .with_model("claude-3-haiku-20240307")
            .with_max_tokens(2048)
            .with_timeout(Duration::from_secs(30));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "claude-3-haiku-20240307");
        assert_eq!(config.default_max_tokens, 2048);
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"claude-sonnet-4-5-20250929"));
        assert!(MODELS.contains(&"claude-opus-4-5-20250514"));
        assert!(MODELS.contains(&"claude-haiku-4-5-20251001"));
        assert!(MODELS.contains(&"claude-3-5-sonnet-20241022"));
    }

    #[test]
    fn test_message_conversion() {
        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        let (system, converted) = AnthropicProvider::convert_messages(&messages);

        assert_eq!(system, Some("You are helpful".to_string()));
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "user");
        assert_eq!(converted[1].role, "assistant");
    }

    // Security tests

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("sk-ant-1234567890abcdefghij");
        assert!(masked.starts_with("sk-a"));
        assert!(masked.contains("..."));
        assert!(!masked.contains("1234567890"));
    }

    #[test]
    fn test_sanitize_api_error() {
        let sanitized = sanitize_api_error("Invalid x-api-key header");
        assert!(!sanitized.contains("x-api-key"));
        assert!(sanitized.contains("authentication"));

        let sanitized = sanitize_api_error("overloaded: too many requests");
        assert!(sanitized.contains("rate limit"));
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = AnthropicConfig::new("sk-ant-1234567890abcdefghij");
        let debug_str = format!("{:?}", config);

        assert!(!debug_str.contains("1234567890"));
        assert!(debug_str.contains("sk-a...ghij"));
    }
}
