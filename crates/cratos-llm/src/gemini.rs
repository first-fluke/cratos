//! Gemini - Google Gemini API provider
//!
//! This module implements the Google Gemini provider using reqwest.

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

/// Sanitize Gemini API error messages to prevent leaking sensitive information
fn sanitize_api_error(error: &str) -> String {
    let lower = error.to_lowercase();

    // Don't expose authentication details
    if lower.contains("api key")
        || lower.contains("apikey")
        || lower.contains("invalid key")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
        || lower.contains("permission denied")
    {
        return "API authentication error. Please check your API key configuration.".to_string();
    }

    // Don't expose rate limit details
    if lower.contains("rate limit")
        || lower.contains("quota")
        || lower.contains("resource_exhausted")
    {
        return "API rate limit exceeded. Please try again later.".to_string();
    }

    // Don't expose internal server errors
    if lower.contains("internal") || lower.contains("server error") {
        return "API server error. Please try again later.".to_string();
    }

    // For short, generic errors without keys, return as-is
    if error.len() < 100 && !error.contains("key=") && !error.contains("key ") {
        return error.to_string();
    }

    "An API error occurred. Please try again.".to_string()
}

/// Available Gemini models
pub const MODELS: &[&str] = &[
    "gemini-1.5-pro",
    "gemini-1.5-flash",
    "gemini-1.5-flash-8b",
    "gemini-2.0-flash-exp",
];

/// Default Gemini model
pub const DEFAULT_MODEL: &str = "gemini-1.5-flash";

/// Default API base URL
const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

// ============================================================================
// API Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_config: Option<ToolConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: FunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: FunctionResponse,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct FunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct FunctionResponse {
    name: String,
    response: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiTool {
    function_declarations: Vec<FunctionDeclaration>,
}

#[derive(Debug, Serialize)]
struct FunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolConfig {
    function_calling_config: FunctionCallingConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FunctionCallingConfig {
    mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_function_names: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Vec<Candidate>,
    #[serde(default)]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    content: GeminiContent,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    prompt_token_count: u32,
    candidates_token_count: u32,
    total_token_count: u32,
}

#[derive(Debug, Deserialize)]
struct GeminiError {
    error: GeminiErrorDetail,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
struct GeminiErrorDetail {
    code: i32,
    message: String,
    status: String,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// Gemini provider configuration
#[derive(Clone)]
pub struct GeminiConfig {
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
impl fmt::Debug for GeminiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeminiConfig")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("default_max_tokens", &self.default_max_tokens)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl GeminiConfig {
    /// Create a new configuration with an API key
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            default_max_tokens: 8192,
            timeout: Duration::from_secs(60),
        }
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GOOGLE_API_KEY")
            .or_else(|_| std::env::var("GEMINI_API_KEY"))
            .map_err(|_| {
                Error::NotConfigured("GOOGLE_API_KEY or GEMINI_API_KEY not set".to_string())
            })?;

        let base_url =
            std::env::var("GEMINI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let default_model =
            std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Ok(Self {
            api_key,
            base_url,
            default_model,
            default_max_tokens: 8192,
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

/// Google Gemini provider
pub struct GeminiProvider {
    client: Client,
    config: GeminiConfig,
}

impl GeminiProvider {
    /// Create a new Gemini provider
    pub fn new(config: GeminiConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;

        Ok(Self { client, config })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = GeminiConfig::from_env()?;
        Self::new(config)
    }

    /// Convert messages to Gemini format, returning system instruction separately
    fn convert_messages(messages: &[Message]) -> (Option<GeminiContent>, Vec<GeminiContent>) {
        let mut system_instruction = None;
        let mut gemini_contents = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    system_instruction = Some(GeminiContent {
                        role: None,
                        parts: vec![GeminiPart::Text {
                            text: msg.content.clone(),
                        }],
                    });
                }
                MessageRole::User => {
                    gemini_contents.push(GeminiContent {
                        role: Some("user".to_string()),
                        parts: vec![GeminiPart::Text {
                            text: msg.content.clone(),
                        }],
                    });
                }
                MessageRole::Assistant => {
                    gemini_contents.push(GeminiContent {
                        role: Some("model".to_string()),
                        parts: vec![GeminiPart::Text {
                            text: msg.content.clone(),
                        }],
                    });
                }
                MessageRole::Tool => {
                    if let Some(tool_name) = &msg.name {
                        // Parse the content as JSON for the response
                        let response_value = serde_json::from_str(&msg.content)
                            .unwrap_or_else(|_| serde_json::json!({"result": msg.content}));

                        gemini_contents.push(GeminiContent {
                            role: Some("user".to_string()),
                            parts: vec![GeminiPart::FunctionResponse {
                                function_response: FunctionResponse {
                                    name: tool_name.clone(),
                                    response: response_value,
                                },
                            }],
                        });
                    }
                }
            }
        }

        (system_instruction, gemini_contents)
    }

    /// Convert tool definitions to Gemini format
    fn convert_tools(tools: &[ToolDefinition]) -> Vec<GeminiTool> {
        let declarations: Vec<FunctionDeclaration> = tools
            .iter()
            .map(|tool| FunctionDeclaration {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect();

        vec![GeminiTool {
            function_declarations: declarations,
        }]
    }

    /// Convert tool choice to Gemini format
    fn convert_tool_choice(choice: &ToolChoice, _tools: &[ToolDefinition]) -> Option<ToolConfig> {
        match choice {
            ToolChoice::Auto => Some(ToolConfig {
                function_calling_config: FunctionCallingConfig {
                    mode: "AUTO".to_string(),
                    allowed_function_names: None,
                },
            }),
            ToolChoice::None => Some(ToolConfig {
                function_calling_config: FunctionCallingConfig {
                    mode: "NONE".to_string(),
                    allowed_function_names: None,
                },
            }),
            ToolChoice::Required => Some(ToolConfig {
                function_calling_config: FunctionCallingConfig {
                    mode: "ANY".to_string(),
                    allowed_function_names: None,
                },
            }),
            ToolChoice::Tool(name) => Some(ToolConfig {
                function_calling_config: FunctionCallingConfig {
                    mode: "ANY".to_string(),
                    allowed_function_names: Some(vec![name.clone()]),
                },
            }),
        }
    }

    /// Send request to Gemini API
    async fn send_request(&self, model: &str, request: GeminiRequest) -> Result<GeminiResponse> {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.config.base_url, model, self.config.api_key
        );

        // SECURITY: Don't log the full URL (contains API key)
        debug!("Sending request to Gemini model: {}", model);

        let response = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !status.is_success() {
            if let Ok(error) = serde_json::from_str::<GeminiError>(&body) {
                if status.as_u16() == 429 {
                    return Err(Error::RateLimit);
                }
                // SECURITY: Sanitize error messages
                return Err(Error::Api(sanitize_api_error(&format!(
                    "{}: {}",
                    error.error.status, error.error.message
                ))));
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
impl LlmProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
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

        let (system_instruction, contents) = Self::convert_messages(&request.messages);

        let generation_config = Some(GenerationConfig {
            temperature: request.temperature,
            max_output_tokens: request.max_tokens.or(Some(self.config.default_max_tokens)),
            stop_sequences: request.stop.clone(),
        });

        let gemini_request = GeminiRequest {
            contents,
            system_instruction,
            generation_config,
            tools: None,
            tool_config: None,
        };

        let response = self.send_request(model, gemini_request).await?;

        let candidate = response
            .candidates
            .first()
            .ok_or_else(|| Error::InvalidResponse("No candidates in response".to_string()))?;

        // Extract text content
        let content = candidate
            .content
            .parts
            .iter()
            .filter_map(|part| match part {
                GeminiPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let usage = response.usage_metadata.map(|u| TokenUsage {
            prompt_tokens: u.prompt_token_count,
            completion_tokens: u.candidates_token_count,
            total_tokens: u.total_token_count,
        });

        Ok(CompletionResponse {
            content,
            usage,
            finish_reason: candidate.finish_reason.clone(),
            model: model.to_string(),
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

        let (system_instruction, contents) = Self::convert_messages(&request.request.messages);

        let generation_config = Some(GenerationConfig {
            temperature: request.request.temperature,
            max_output_tokens: request
                .request
                .max_tokens
                .or(Some(self.config.default_max_tokens)),
            stop_sequences: request.request.stop.clone(),
        });

        let tools = Self::convert_tools(&request.tools);
        let tool_config = Self::convert_tool_choice(&request.tool_choice, &request.tools);

        let gemini_request = GeminiRequest {
            contents,
            system_instruction,
            generation_config,
            tools: Some(tools),
            tool_config,
        };

        let response = self.send_request(model, gemini_request).await?;

        let candidate = response
            .candidates
            .first()
            .ok_or_else(|| Error::InvalidResponse("No candidates in response".to_string()))?;

        // Extract text content and tool calls
        let mut content = None;
        let mut tool_calls = Vec::new();

        for part in &candidate.content.parts {
            match part {
                GeminiPart::Text { text } => {
                    content = Some(text.clone());
                }
                GeminiPart::FunctionCall { function_call } => {
                    tool_calls.push(ToolCall {
                        id: uuid::Uuid::new_v4().to_string(), // Gemini doesn't provide IDs
                        name: function_call.name.clone(),
                        arguments: serde_json::to_string(&function_call.args)
                            .unwrap_or_else(|_| "{}".to_string()),
                    });
                }
                _ => {}
            }
        }

        let usage = response.usage_metadata.map(|u| TokenUsage {
            prompt_tokens: u.prompt_token_count,
            completion_tokens: u.candidates_token_count,
            total_tokens: u.total_token_count,
        });

        Ok(ToolCompletionResponse {
            content,
            tool_calls,
            usage,
            finish_reason: candidate.finish_reason.clone(),
            model: model.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = GeminiConfig::new("test-key")
            .with_model("gemini-1.5-pro")
            .with_max_tokens(4096)
            .with_timeout(Duration::from_secs(30));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "gemini-1.5-pro");
        assert_eq!(config.default_max_tokens, 4096);
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"gemini-1.5-pro"));
        assert!(MODELS.contains(&"gemini-1.5-flash"));
    }

    #[test]
    fn test_message_conversion() {
        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        let (system, converted) = GeminiProvider::convert_messages(&messages);

        assert!(system.is_some());
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, Some("user".to_string()));
        assert_eq!(converted[1].role, Some("model".to_string()));
    }

    // Security tests

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("AIza1234567890abcdefghij");
        assert!(masked.starts_with("AIza"));
        assert!(masked.contains("..."));
        assert!(!masked.contains("1234567890"));
    }

    #[test]
    fn test_sanitize_api_error() {
        let sanitized = sanitize_api_error("Permission denied: invalid API key");
        assert!(!sanitized.contains("invalid"));
        assert!(sanitized.contains("authentication"));

        let sanitized = sanitize_api_error("RESOURCE_EXHAUSTED: quota exceeded");
        assert!(sanitized.contains("rate limit"));
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = GeminiConfig::new("AIza1234567890abcdefghij");
        let debug_str = format!("{:?}", config);

        assert!(!debug_str.contains("1234567890"));
        assert!(debug_str.contains("AIza...ghij"));
    }
}
