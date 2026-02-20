use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Default Ollama models (varies by installation)
pub const SUGGESTED_MODELS: &[&str] = &[
    "qwen2.5:7b",
    "llama3.1:8b",
    "gemma2:9b",
    "mistral:7b",
    "mixtral",
    "codellama",
    "phi3",
];

/// Default Ollama model (7B+ recommended for tool calling)
pub const DEFAULT_MODEL: &str = "qwen2.5:7b";

/// Default Ollama API URL
pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";

// ============================================================================
// API Types
// ============================================================================

/// Request for the Ollama chat endpoint
#[derive(Debug, Serialize)]
pub struct OllamaChatRequest {
    /// The model name to use
    pub model: String,
    /// List of messages in the conversation
    pub messages: Vec<OllamaMessage>,
    /// Additional model options (temperature, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<OllamaOptions>,
    /// Whether to stream the response
    pub stream: bool,
    /// Tools available for the model to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OllamaTool>>,
}

/// Message format for Ollama chat
#[derive(Debug, Serialize)]
pub struct OllamaMessage {
    /// Role of the message sender (system, user, assistant, tool)
    pub role: String,
    /// Content of the message
    pub content: String,
    /// Tool calls made by the assistant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
}

/// Model configuration options for Ollama
#[derive(Debug, Serialize)]
pub struct OllamaOptions {
    /// Sampling temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Maximum number of tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<u32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

/// Tool definition for Ollama
#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaTool {
    /// Type of tool (typically "function")
    pub r#type: String,
    /// Function details
    pub function: OllamaFunction,
}

/// Function definition for a tool
#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaFunction {
    /// Name of the function
    pub name: String,
    /// Description of what the function does
    pub description: String,
    /// JSON schema of the parameters
    pub parameters: serde_json::Value,
}

/// A tool call made by the model
#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaToolCall {
    /// The function being called
    pub function: OllamaFunctionCall,
}

/// Specific function call details
#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaFunctionCall {
    /// Name of the function to call
    pub name: String,
    /// Arguments for the function call
    pub arguments: serde_json::Value,
}

/// Response from the Ollama chat endpoint
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
pub struct OllamaChatResponse {
    /// The model used to generate the response
    pub model: String,
    /// The generated message
    pub message: OllamaResponseMessage,
    /// Whether generation is finished
    pub done: bool,
    /// Why generation stopped
    #[serde(default)]
    pub done_reason: Option<String>,
    /// Total time spent processing the request (in nanoseconds)
    #[serde(default)]
    pub total_duration: Option<u64>,
    /// Number of tokens in the prompt
    #[serde(default)]
    pub prompt_eval_count: Option<u32>,
    /// Number of tokens generated
    #[serde(default)]
    pub eval_count: Option<u32>,
}

/// Message format in Ollama responses
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde for JSON deserialization
pub struct OllamaResponseMessage {
    /// Role of the message sender
    pub role: String,
    /// Content of the message
    pub content: String,
    /// Tool calls requested by the model
    #[serde(default)]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
}

/// Error response from Ollama API
#[derive(Debug, Deserialize)]
pub struct OllamaError {
    /// Error message
    pub error: String,
}

/// Response from the /api/tags endpoint
#[derive(Debug, Deserialize)]
pub struct OllamaTagsResponse {
    /// List of available models
    pub models: Vec<OllamaModel>,
}

/// Model information from the tags response
#[derive(Debug, Deserialize)]
pub struct OllamaModel {
    /// Name of the model
    pub name: String,
}

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
