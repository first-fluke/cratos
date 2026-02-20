use reqwest::Client;
use tracing::{debug, instrument};
use crate::error::{Error, Result};
use crate::router::{
    CompletionRequest, CompletionResponse, LlmProvider, TokenUsage, ToolCall,
    ToolCompletionRequest, ToolCompletionResponse,
};
use super::types::{
    AnthropicConfig, AnthropicRequest, AnthropicResponse, AnthropicError,
    ResponseContentBlock, API_VERSION, MODELS,
};
use super::convert::{convert_messages, convert_tool, convert_tool_choice};
use super::security::sanitize_api_error;

/// Anthropic Claude provider
pub struct AnthropicProvider {
    pub(crate) client: Client,
    pub(crate) config: AnthropicConfig,
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

        let (system, messages) = convert_messages(&request.messages);

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

        let (system, messages) = convert_messages(&request.request.messages);

        let tools: Vec<_> = request.tools.iter().map(convert_tool).collect();

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
            tool_choice: convert_tool_choice(&request.tool_choice),
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
