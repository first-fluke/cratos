//! Ollama - Local Ollama API provider
//!
//! This module implements the Ollama provider for local LLM inference.
//! Ollama runs models locally and provides an OpenAI-compatible API.

use crate::error::{Error, Result};
use crate::router::{
    CompletionRequest, CompletionResponse, LlmProvider, Message, MessageRole, TokenUsage, ToolCall,
    ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, instrument};

// ============================================================================
// Security Utilities
// ============================================================================

/// Sanitize API error messages to prevent leaking sensitive information
fn sanitize_api_error(error: &str) -> String {
    let lower = error.to_lowercase();

    // Don't expose internal paths or system information
    if lower.contains("/home")
        || lower.contains("/root")
        || lower.contains("/var")
        || lower.contains("\\users\\")
    {
        return "An internal error occurred. Please check your Ollama installation.".to_string();
    }

    // For connection errors, provide helpful message
    if lower.contains("connection refused") || lower.contains("failed to connect") {
        return "Failed to connect to Ollama. Is Ollama running?".to_string();
    }

    // For model errors
    if lower.contains("model") && (lower.contains("not found") || lower.contains("pull")) {
        return "Model not available. Please pull the model first with: ollama pull <model>"
            .to_string();
    }

    // For short, generic errors, return as-is
    if error.len() < 100 {
        return error.to_string();
    }

    "An error occurred. Please try again.".to_string()
}

/// Default Ollama models (varies by installation)
pub const SUGGESTED_MODELS: &[&str] = &[
    "llama3.2",
    "llama3.1",
    "llama3",
    "mistral",
    "mixtral",
    "codellama",
    "phi3",
    "qwen2.5",
];

/// Default Ollama model
pub const DEFAULT_MODEL: &str = "llama3.2";

/// Default Ollama API URL
const DEFAULT_BASE_URL: &str = "http://localhost:11434";

// ============================================================================
// API Types
// ============================================================================

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaTool {
    r#type: String,
    function: OllamaFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaFunctionCall {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct OllamaChatResponse {
    model: String,
    message: OllamaResponseMessage,
    done: bool,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct OllamaResponseMessage {
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OllamaError {
    error: String,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// Ollama provider configuration
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// Base URL (default: http://localhost:11434)
    pub base_url: String,
    /// Default model
    pub default_model: String,
    /// Default max tokens
    pub default_max_tokens: u32,
    /// Request timeout (longer for local inference)
    pub timeout: Duration,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            default_max_tokens: 4096,
            timeout: Duration::from_secs(300), // 5 minutes for local inference
        }
    }
}

impl OllamaConfig {
    /// Create a new configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let base_url = std::env::var("OLLAMA_BASE_URL")
            .or_else(|_| std::env::var("OLLAMA_HOST"))
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        let default_model =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Self {
            base_url,
            default_model,
            default_max_tokens: 4096,
            timeout: Duration::from_secs(300),
        }
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

    /// Convert messages to Ollama format
    fn convert_messages(messages: &[Message]) -> Vec<OllamaMessage> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "tool",
                };

                OllamaMessage {
                    role: role.to_string(),
                    content: msg.content.clone(),
                    tool_calls: None,
                }
            })
            .collect()
    }

    /// Convert tool definitions to Ollama format
    fn convert_tools(tools: &[ToolDefinition]) -> Vec<OllamaTool> {
        tools
            .iter()
            .map(|tool| OllamaTool {
                r#type: "function".to_string(),
                function: OllamaFunction {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.clone(),
                },
            })
            .collect()
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
                return Err(Error::Api(sanitize_api_error(&error.error)));
            }
            // SECURITY: Don't expose raw HTTP response body
            return Err(Error::Api(sanitize_api_error(&format!(
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

        let messages = Self::convert_messages(&request.messages);

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

        let messages = Self::convert_messages(&request.request.messages);
        let tools = Self::convert_tools(&request.tools);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = OllamaConfig::new()
            .with_model("mistral")
            .with_base_url("http://192.168.1.100:11434")
            .with_max_tokens(2048)
            .with_timeout(Duration::from_secs(120));

        assert_eq!(config.default_model, "mistral");
        assert_eq!(config.base_url, "http://192.168.1.100:11434");
        assert_eq!(config.default_max_tokens, 2048);
        assert_eq!(config.timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_default_config() {
        let config = OllamaConfig::default();

        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.default_model, DEFAULT_MODEL);
        assert_eq!(config.timeout, Duration::from_secs(300));
    }

    #[test]
    fn test_message_conversion() {
        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        let converted = OllamaProvider::convert_messages(&messages);

        assert_eq!(converted.len(), 3);
        assert_eq!(converted[0].role, "system");
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[2].role, "assistant");
    }

    // Security tests

    #[test]
    fn test_sanitize_api_error() {
        // Path exposure should be sanitized
        let sanitized = sanitize_api_error("Error loading model from /home/user/.ollama/models");
        assert!(!sanitized.contains("/home"));
        assert!(sanitized.contains("installation"));

        // Connection errors should give helpful message
        let sanitized = sanitize_api_error("connection refused");
        assert!(sanitized.contains("Ollama running"));

        // Model errors should suggest pull
        let sanitized = sanitize_api_error("model 'llama3' not found");
        assert!(sanitized.contains("pull"));
    }
}
