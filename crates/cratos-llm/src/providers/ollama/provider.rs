use crate::error::{Error, Result};
use crate::router::{
    CompletionRequest, CompletionResponse, LlmProvider, TokenUsage, ToolCall,
    ToolCompletionRequest, ToolCompletionResponse,
};
use crate::providers::ollama::{
    convert, security,
    types::{
        OllamaChatRequest, OllamaChatResponse, OllamaConfig, OllamaError, OllamaOptions,
        OllamaTagsResponse, SUGGESTED_MODELS,
    },
};
use reqwest::Client;
use tracing::{debug, instrument};

/// Ollama local provider
pub struct OllamaProvider {
    client: Client,
    config: OllamaConfig,
    /// Cached list of available models
    cached_models: std::sync::RwLock<Vec<String>>,
}

impl OllamaProvider {
    /// Create a new Ollama provider
    pub fn new(config: OllamaConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;

        Ok(Self {
            client,
            config,
            cached_models: std::sync::RwLock::new(Vec::new()),
        })
    }

    /// Create with default configuration
    pub fn with_defaults() -> Result<Self> {
        Self::new(OllamaConfig::default())
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = OllamaConfig::from_env();
        Self::new(config)
    }

    /// Check if Ollama is available
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.config.base_url);
        self.client.get(&url).send().await.is_ok()
    }

    /// List available models from Ollama
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.config.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to connect to Ollama: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Api(format!(
                "Ollama returned status {}",
                response.status()
            )));
        }

        let tags: OllamaTagsResponse = response
            .json()
            .await
            .map_err(|e| Error::InvalidResponse(e.to_string()))?;

        let models: Vec<String> = tags.models.into_iter().map(|m| m.name).collect();

        // Update cache
        if let Ok(mut cache) = self.cached_models.write() {
            *cache = models.clone();
        }

        Ok(models)
    }

    /// Send request to Ollama API
    async fn send_request(&self, request: OllamaChatRequest) -> Result<OllamaChatResponse> {
        let url = format!("{}/api/chat", self.config.base_url);

        debug!("Sending request to Ollama: {}", request.model);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    Error::Network(format!(
                        "Failed to connect to Ollama at {}. Is Ollama running?",
                        self.config.base_url
                    ))
                } else if e.is_timeout() {
                    Error::Timeout(self.config.timeout.as_millis() as u64)
                } else {
                    Error::Network(e.to_string())
                }
            })?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !status.is_success() {
            if let Ok(error) = serde_json::from_str::<OllamaError>(&body) {
                // SECURITY: Sanitize error messages
                return Err(Error::Api(security::sanitize_api_error(&error.error)));
            }
            // SECURITY: Don't expose raw HTTP response body
            return Err(Error::Api(security::sanitize_api_error(&format!(
                "HTTP {}: {}",
                status, body
            ))));
        }

        serde_json::from_str(&body).map_err(|e| Error::InvalidResponse(format!("{}: {}", e, body)))
    }
}

#[async_trait::async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn supports_tools(&self) -> bool {
        // Ollama supports tools for some models (llama3.1+, mistral, etc.)
        // but not all. We return true and let the API handle unsupported cases.
        true
    }

    fn available_models(&self) -> Vec<String> {
        // Return cached models or suggested defaults
        if let Ok(cache) = self.cached_models.read() {
            if !cache.is_empty() {
                return cache.clone();
            }
        }
        SUGGESTED_MODELS.iter().map(|s| (*s).to_string()).collect()
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

        let messages = convert::convert_messages(&request.messages);

        let options = Some(OllamaOptions {
            temperature: request.temperature,
            num_predict: request.max_tokens.or(Some(self.config.default_max_tokens)),
            stop: request.stop.clone(),
        });

        let ollama_request = OllamaChatRequest {
            model: model.to_string(),
            messages,
            options,
            stream: false,
            tools: None,
        };

        let response = self.send_request(ollama_request).await?;

        let usage = match (response.prompt_eval_count, response.eval_count) {
            (Some(prompt), Some(completion)) => Some(TokenUsage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: prompt + completion,
            }),
            _ => None,
        };

        Ok(CompletionResponse {
            content: response.message.content,
            usage,
            finish_reason: response.done_reason,
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

        let messages = convert::convert_messages(&request.request.messages);
        let tools = convert::convert_tools(&request.tools);

        let options = Some(OllamaOptions {
            temperature: request.request.temperature,
            num_predict: request
                .request
                .max_tokens
                .or(Some(self.config.default_max_tokens)),
            stop: request.request.stop.clone(),
        });

        let ollama_request = OllamaChatRequest {
            model: model.to_string(),
            messages,
            options,
            stream: false,
            tools: Some(tools),
        };

        let response = self.send_request(ollama_request).await?;

        // Extract tool calls
        let tool_calls: Vec<ToolCall> = response
            .message
            .tool_calls
            .map(|calls| {
                calls
                    .into_iter()
                    .enumerate()
                    .map(|(i, tc)| ToolCall {
                        id: format!("call_{}", i), // Ollama doesn't provide IDs
                        name: tc.function.name,
                        arguments: serde_json::to_string(&tc.function.arguments)
                            .unwrap_or_else(|_| "{}".to_string()),
                        thought_signature: None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let content = if response.message.content.is_empty() {
            None
        } else {
            Some(response.message.content)
        };

        let usage = match (response.prompt_eval_count, response.eval_count) {
            (Some(prompt), Some(completion)) => Some(TokenUsage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: prompt + completion,
            }),
            _ => None,
        };

        Ok(ToolCompletionResponse {
            content,
            tool_calls,
            usage,
            finish_reason: response.done_reason,
            model: response.model,
        })
    }
}
