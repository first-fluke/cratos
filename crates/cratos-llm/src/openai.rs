//! OpenAI - async-openai provider
//!
//! This module implements the OpenAI LLM provider using async-openai.

use crate::error::{Error, Result};
use crate::router::{
    CompletionRequest, CompletionResponse, LlmProvider, Message, MessageRole, TokenUsage, ToolCall,
    ToolChoice, ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
};
use crate::util::mask_api_key;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
        ChatCompletionRequestUserMessageArgs, ChatCompletionTool, ChatCompletionToolArgs,
        ChatCompletionToolChoiceOption, ChatCompletionToolType, CreateChatCompletionRequestArgs,
        FunctionObjectArgs,
    },
    Client,
};
use std::fmt;
use std::time::Duration;
use tracing::{debug, instrument};

// ============================================================================
// Security Utilities
// ============================================================================

/// Sanitize API error messages to prevent leaking sensitive information
fn sanitize_api_error(error: &str) -> String {
    let lower = error.to_lowercase();

    // Don't expose authentication details
    if lower.contains("api key")
        || lower.contains("apikey")
        || lower.contains("invalid key")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
    {
        return "API authentication error. Please check your API key configuration.".to_string();
    }

    // Don't expose rate limit details that could be exploited
    if lower.contains("rate limit") || lower.contains("quota") {
        return "API rate limit exceeded. Please try again later.".to_string();
    }

    // Don't expose internal server errors with details
    if lower.contains("internal") || lower.contains("server error") {
        return "API server error. Please try again later.".to_string();
    }

    // For short, generic errors, return as-is
    if error.len() < 100 && !error.contains("sk-") && !error.contains("key") {
        return error.to_string();
    }

    "An API error occurred. Please try again.".to_string()
}

/// Available OpenAI models
pub const MODELS: &[&str] = &[
    "gpt-4o",
    "gpt-4o-mini",
    "gpt-4-turbo",
    "gpt-4",
    "gpt-3.5-turbo",
];

/// Default OpenAI model
pub const DEFAULT_MODEL: &str = "gpt-4o";

/// OpenAI provider configuration
#[derive(Clone)]
pub struct OpenAiConfig {
    /// API key
    pub api_key: String,
    /// Base URL (optional, for Azure or other endpoints)
    pub base_url: Option<String>,
    /// Organization ID (optional)
    pub org_id: Option<String>,
    /// Default model
    pub default_model: String,
    /// Request timeout
    pub timeout: Duration,
}

// SECURITY: Custom Debug implementation to mask API key
impl fmt::Debug for OpenAiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiConfig")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("base_url", &self.base_url)
            .field("org_id", &self.org_id.as_ref().map(|_| "[REDACTED]"))
            .field("default_model", &self.default_model)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl OpenAiConfig {
    /// Create a new configuration with an API key
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: None,
            org_id: None,
            default_model: DEFAULT_MODEL.to_string(),
            timeout: Duration::from_secs(60),
        }
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| Error::NotConfigured("OPENAI_API_KEY not set".to_string()))?;

        let base_url = std::env::var("OPENAI_BASE_URL").ok();
        let org_id = std::env::var("OPENAI_ORG_ID").ok();
        let default_model =
            std::env::var("OPENAI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Ok(Self {
            api_key,
            base_url,
            org_id,
            default_model,
            timeout: Duration::from_secs(60),
        })
    }

    /// Set the base URL
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the organization ID
    #[must_use]
    pub fn with_org_id(mut self, org_id: impl Into<String>) -> Self {
        self.org_id = Some(org_id.into());
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

/// OpenAI provider
pub struct OpenAiProvider {
    client: Client<OpenAIConfig>,
    default_model: String,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider
    #[must_use]
    pub fn new(config: OpenAiConfig) -> Self {
        let mut openai_config = OpenAIConfig::new().with_api_key(&config.api_key);

        if let Some(base_url) = &config.base_url {
            openai_config = openai_config.with_api_base(base_url);
        }

        if let Some(org_id) = &config.org_id {
            openai_config = openai_config.with_org_id(org_id);
        }

        let client = Client::with_config(openai_config);

        Self {
            client,
            default_model: config.default_model,
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = OpenAiConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Convert our message to OpenAI format
    fn convert_message(msg: &Message) -> Result<ChatCompletionRequestMessage> {
        let message = match msg.role {
            MessageRole::System => ChatCompletionRequestSystemMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .map_err(|e| Error::InvalidResponse(e.to_string()))?
                .into(),
            MessageRole::User => ChatCompletionRequestUserMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .map_err(|e| Error::InvalidResponse(e.to_string()))?
                .into(),
            MessageRole::Assistant => ChatCompletionRequestAssistantMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .map_err(|e| Error::InvalidResponse(e.to_string()))?
                .into(),
            MessageRole::Tool => {
                let tool_call_id = msg.tool_call_id.as_ref().ok_or_else(|| {
                    Error::InvalidResponse("Tool message missing tool_call_id".to_string())
                })?;
                ChatCompletionRequestToolMessageArgs::default()
                    .content(msg.content.clone())
                    .tool_call_id(tool_call_id)
                    .build()
                    .map_err(|e| Error::InvalidResponse(e.to_string()))?
                    .into()
            }
        };
        Ok(message)
    }

    /// Convert tool definition to OpenAI format
    fn convert_tool(tool: &ToolDefinition) -> Result<ChatCompletionTool> {
        let function = FunctionObjectArgs::default()
            .name(&tool.name)
            .description(&tool.description)
            .parameters(tool.parameters.clone())
            .build()
            .map_err(|e| Error::InvalidResponse(e.to_string()))?;

        ChatCompletionToolArgs::default()
            .r#type(ChatCompletionToolType::Function)
            .function(function)
            .build()
            .map_err(|e| Error::InvalidResponse(e.to_string()))
    }

    /// Convert tool choice to OpenAI format
    fn convert_tool_choice(choice: &ToolChoice) -> ChatCompletionToolChoiceOption {
        match choice {
            ToolChoice::Auto => ChatCompletionToolChoiceOption::Auto,
            ToolChoice::None => ChatCompletionToolChoiceOption::None,
            ToolChoice::Required | ToolChoice::Tool(_) => {
                // async-openai 0.18 doesn't have Required variant
                // Use Auto as fallback
                ChatCompletionToolChoiceOption::Auto
            }
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn available_models(&self) -> Vec<String> {
        MODELS.iter().map(|s| (*s).to_string()).collect()
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let model = if request.model.is_empty() {
            &self.default_model
        } else {
            &request.model
        };

        let messages: Vec<ChatCompletionRequestMessage> = request
            .messages
            .iter()
            .map(Self::convert_message)
            .collect::<Result<_>>()?;

        let mut builder = CreateChatCompletionRequestArgs::default();
        builder.model(model).messages(messages);

        if let Some(max_tokens) = request.max_tokens {
            builder.max_tokens(max_tokens as u16);
        }

        if let Some(temperature) = request.temperature {
            builder.temperature(temperature);
        }

        if let Some(stop) = &request.stop {
            builder.stop(stop);
        }

        let openai_request = builder
            .build()
            .map_err(|e| Error::InvalidResponse(e.to_string()))?;

        debug!("Sending request to OpenAI");

        let response = self
            .client
            .chat()
            .create(openai_request)
            .await
            .map_err(|e| Error::Api(sanitize_api_error(&e.to_string())))?;

        let choice = response
            .choices
            .first()
            .ok_or_else(|| Error::InvalidResponse("No choices in response".to_string()))?;

        let content = choice.message.content.clone().unwrap_or_default();

        let usage = response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(CompletionResponse {
            content,
            usage,
            finish_reason: choice.finish_reason.as_ref().map(|r| format!("{:?}", r)),
            model: response.model,
        })
    }

    #[instrument(skip(self, request), fields(model = %request.request.model, tools = request.tools.len()))]
    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse> {
        let model = if request.request.model.is_empty() {
            &self.default_model
        } else {
            &request.request.model
        };

        let messages: Vec<ChatCompletionRequestMessage> = request
            .request
            .messages
            .iter()
            .map(Self::convert_message)
            .collect::<Result<_>>()?;

        let tools: Vec<ChatCompletionTool> = request
            .tools
            .iter()
            .map(Self::convert_tool)
            .collect::<Result<_>>()?;

        let mut builder = CreateChatCompletionRequestArgs::default();
        builder
            .model(model)
            .messages(messages)
            .tools(tools)
            .tool_choice(Self::convert_tool_choice(&request.tool_choice));

        if let Some(max_tokens) = request.request.max_tokens {
            builder.max_tokens(max_tokens as u16);
        }

        if let Some(temperature) = request.request.temperature {
            builder.temperature(temperature);
        }

        let openai_request = builder
            .build()
            .map_err(|e| Error::InvalidResponse(e.to_string()))?;

        debug!("Sending tool request to OpenAI");

        let response = self
            .client
            .chat()
            .create(openai_request)
            .await
            .map_err(|e| Error::Api(sanitize_api_error(&e.to_string())))?;

        let choice = response
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

        let usage = response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ToolCompletionResponse {
            content,
            tool_calls,
            usage,
            finish_reason: choice.finish_reason.as_ref().map(|r| format!("{:?}", r)),
            model: response.model,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::mask_api_key;

    #[test]
    fn test_config_builder() {
        let config = OpenAiConfig::new("test-key")
            .with_model("gpt-4o-mini")
            .with_timeout(Duration::from_secs(30));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "gpt-4o-mini");
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"gpt-4o"));
        assert!(MODELS.contains(&"gpt-4o-mini"));
    }

    // Security tests

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("sk-1234567890abcdefghijklmnop");
        assert!(masked.starts_with("sk-1"));
        assert!(masked.ends_with("mnop"));
        assert!(masked.contains("..."));
        assert!(!masked.contains("567890abcdefghijkl"));
    }

    #[test]
    fn test_short_key_masking() {
        let masked = mask_api_key("short");
        assert_eq!(masked, "****");
    }

    #[test]
    fn test_sanitize_api_error() {
        // Auth errors should be sanitized
        let sanitized = sanitize_api_error("Invalid API key: sk-1234567890");
        assert!(!sanitized.contains("sk-"));
        assert!(sanitized.contains("authentication"));

        // Rate limit errors should be sanitized
        let sanitized = sanitize_api_error("Rate limit exceeded: 100 requests per minute");
        assert!(!sanitized.contains("100"));
        assert!(sanitized.contains("rate limit"));

        // Short generic errors can pass through
        let sanitized = sanitize_api_error("Model not found");
        assert_eq!(sanitized, "Model not found");
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = OpenAiConfig::new("sk-1234567890abcdefghijklmnop");
        let debug_str = format!("{:?}", config);

        // Should not contain the full API key
        assert!(!debug_str.contains("1234567890abcdefghijkl"));
        // Should contain masked version
        assert!(debug_str.contains("sk-1...mnop"));
    }
}
