//! OpenAI - async-openai provider
//!
//! This module implements the OpenAI LLM provider using async-openai 0.32+.

use crate::cli_auth::{self, AuthSource};
use crate::error::{Error, Result};
use crate::router::{
    CompletionRequest, CompletionResponse, LlmProvider, Message, MessageRole, TokenUsage, ToolCall,
    ToolChoice, ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
};
use crate::util::mask_api_key;
use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestSystemMessageContent,
        ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
        ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
        ChatCompletionTool, ChatCompletionToolChoiceOption, ChatCompletionTools,
        CreateChatCompletionRequest, FunctionObject, StopConfiguration, ToolChoiceOptions,
    },
    Client,
};
use std::fmt;
use std::time::Duration;
use tracing::{debug, instrument};

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

/// Available OpenAI models (2026)
///
/// GPT-5 family pricing (per 1M tokens):
/// - gpt-5-nano: $0.05/$0.40 (cheapest, 32K context)
/// - gpt-5: $1.25/$10.00 (400K context, cheaper than GPT-4o!)
/// - gpt-5.2: latest flagship
///
/// GPT-4o family (legacy, still available):
/// - gpt-4o-mini: $0.15/$0.60 (128K context)
/// - gpt-4o: $2.50/$10.00 (128K context)
pub const MODELS: &[&str] = &[
    // GPT-5 family (2025-08~)
    "gpt-5.2",
    "gpt-5",
    "gpt-5-nano",
    // GPT-4o family (legacy)
    "gpt-4o",
    "gpt-4o-mini",
];

/// Default model — GPT-5 is cheaper ($1.25) than GPT-4o ($2.50) per 1M input tokens
pub const DEFAULT_MODEL: &str = "gpt-5";

/// Configuration for the OpenAI provider
#[derive(Clone)]
pub struct OpenAiConfig {
    /// API key for authentication
    pub api_key: String,
    /// Authentication source (for logging)
    pub auth_source: AuthSource,
    /// Optional custom base URL (for Azure OpenAI or proxies)
    pub base_url: Option<String>,
    /// Optional organization ID
    pub org_id: Option<String>,
    /// Default model to use for completions
    pub default_model: String,
    /// Request timeout duration
    pub timeout: Duration,
}

impl fmt::Debug for OpenAiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiConfig")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("auth_source", &self.auth_source)
            .field("base_url", &self.base_url)
            .field("org_id", &self.org_id.as_ref().map(|_| "[REDACTED]"))
            .field("default_model", &self.default_model)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl OpenAiConfig {
    /// Creates a new configuration with the given API key
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            auth_source: AuthSource::ApiKey,
            base_url: None,
            org_id: None,
            default_model: DEFAULT_MODEL.to_string(),
            timeout: Duration::from_secs(60),
        }
    }

    /// Creates configuration from environment variables.
    ///
    /// Priority:
    /// 1. `OPENAI_API_KEY` env var
    /// 2. `~/.codex/auth.json` (Codex CLI / ChatGPT Pro subscription)
    ///
    /// # Errors
    /// Returns error if neither source is available
    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("OPENAI_BASE_URL").ok();
        let org_id = std::env::var("OPENAI_ORG_ID").ok();
        let default_model =
            std::env::var("OPENAI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        // 1. Try explicit API key
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            return Ok(Self {
                api_key,
                auth_source: AuthSource::ApiKey,
                base_url,
                org_id,
                default_model,
                timeout: Duration::from_secs(60),
            });
        }

        // 2. Try Codex CLI auth credentials
        if let Some(creds) = cli_auth::read_codex_auth() {
            return Ok(Self {
                api_key: creds.tokens.access_token,
                auth_source: AuthSource::CodexCli,
                base_url,
                org_id,
                default_model,
                timeout: Duration::from_secs(60),
            });
        }

        Err(Error::NotConfigured(
            "OPENAI_API_KEY not set and Codex CLI credentials not found".to_string(),
        ))
    }

    /// Sets a custom base URL
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Sets the organization ID
    #[must_use]
    pub fn with_org_id(mut self, org_id: impl Into<String>) -> Self {
        self.org_id = Some(org_id.into());
        self
    }

    /// Sets the default model
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Sets the request timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// OpenAI API provider for chat completions
pub struct OpenAiProvider {
    client: Client<OpenAIConfig>,
    default_model: String,
}

impl OpenAiProvider {
    /// Creates a new provider with the given configuration
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

    /// Creates a provider from environment variables
    ///
    /// # Errors
    /// Returns error if `OPENAI_API_KEY` is not set
    pub fn from_env() -> Result<Self> {
        let config = OpenAiConfig::from_env()?;
        Ok(Self::new(config))
    }

    fn convert_message(msg: &Message) -> Result<ChatCompletionRequestMessage> {
        let message = match msg.role {
            MessageRole::System => ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(msg.content.clone()),
                name: None,
            }
            .into(),
            MessageRole::User => ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(msg.content.clone()),
                name: None,
            }
            .into(),
            MessageRole::Assistant =>
            {
                #[allow(deprecated)]
                ChatCompletionRequestAssistantMessage {
                    content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                        msg.content.clone(),
                    )),
                    name: None,
                    tool_calls: None,
                    function_call: None,
                    refusal: None,
                    audio: None,
                }
                .into()
            }
            MessageRole::Tool => {
                let tool_call_id = msg.tool_call_id.as_ref().ok_or_else(|| {
                    Error::InvalidResponse("Tool message missing tool_call_id".to_string())
                })?;
                ChatCompletionRequestToolMessage {
                    content: ChatCompletionRequestToolMessageContent::Text(msg.content.clone()),
                    tool_call_id: tool_call_id.clone(),
                }
                .into()
            }
        };
        Ok(message)
    }

    fn convert_tool(tool: &ToolDefinition) -> ChatCompletionTool {
        ChatCompletionTool {
            function: FunctionObject {
                name: tool.name.clone(),
                description: Some(tool.description.clone()),
                parameters: Some(tool.parameters.clone()),
                strict: None,
            },
        }
    }

    fn convert_tool_choice(choice: &ToolChoice) -> ChatCompletionToolChoiceOption {
        match choice {
            ToolChoice::Auto => ChatCompletionToolChoiceOption::Mode(ToolChoiceOptions::Auto),
            ToolChoice::None => ChatCompletionToolChoiceOption::Mode(ToolChoiceOptions::None),
            ToolChoice::Required => {
                ChatCompletionToolChoiceOption::Mode(ToolChoiceOptions::Required)
            }
            ToolChoice::Tool(_) => ChatCompletionToolChoiceOption::Mode(ToolChoiceOptions::Auto),
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

        let openai_request = CreateChatCompletionRequest {
            model: model.clone(),
            messages,
            max_completion_tokens: request.max_tokens,
            temperature: request.temperature,
            stop: request.stop.map(StopConfiguration::StringArray),
            ..Default::default()
        };

        debug!("Sending request to OpenAI");

        let response = self.client.chat().create(openai_request).await.map_err(
            |e: async_openai::error::OpenAIError| Error::Api(sanitize_api_error(&e.to_string())),
        )?;

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

        let tools: Vec<ChatCompletionTools> = request
            .tools
            .iter()
            .map(|tool| ChatCompletionTools::Function(Self::convert_tool(tool)))
            .collect();

        let openai_request = CreateChatCompletionRequest {
            model: model.clone(),
            messages,
            tools: Some(tools),
            tool_choice: Some(Self::convert_tool_choice(&request.tool_choice)),
            max_completion_tokens: request.request.max_tokens,
            temperature: request.request.temperature,
            ..Default::default()
        };

        debug!("Sending tool request to OpenAI");

        let response = self.client.chat().create(openai_request).await.map_err(
            |e: async_openai::error::OpenAIError| Error::Api(sanitize_api_error(&e.to_string())),
        )?;

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
                    .filter_map(|tc| match tc {
                        ChatCompletionMessageToolCalls::Function(func_call) => Some(ToolCall {
                            id: func_call.id.clone(),
                            name: func_call.function.name.clone(),
                            arguments: func_call.function.arguments.clone(),
                        }),
                        _ => None,
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
        let sanitized = sanitize_api_error("Invalid API key: sk-1234567890");
        assert!(!sanitized.contains("sk-"));
        assert!(sanitized.contains("authentication"));

        let sanitized = sanitize_api_error("Rate limit exceeded: 100 requests per minute");
        assert!(!sanitized.contains("100"));
        assert!(sanitized.contains("rate limit"));

        let sanitized = sanitize_api_error("Model not found");
        assert_eq!(sanitized, "Model not found");
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = OpenAiConfig::new("sk-1234567890abcdefghijklmnop");
        let debug_str = format!("{:?}", config);

        assert!(!debug_str.contains("1234567890abcdefghijkl"));
        assert!(debug_str.contains("sk-1...mnop"));
    }
}
