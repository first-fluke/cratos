//! Gemini - Google Gemini API provider
//!
//! This module implements the Google Gemini provider using reqwest.

use crate::cli_auth::{self, AuthSource};
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
use tracing::{debug, info, instrument};

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

    // Truncate overly long messages but preserve useful error info
    if error.len() > 300 {
        format!("{}...(truncated)", crate::util::truncate_safe(error, 300))
    } else {
        error.to_string()
    }
}

/// Available Gemini models (2026)
///
/// Pricing per 1M tokens:
/// - gemini-2.5-flash-lite: ~$0.10 (cheapest)
/// - gemini-2.5-flash: $0.075/$0.60 (free tier available!)
/// - gemini-2.5-pro: $1.25/$15.00 (best for coding)
pub const MODELS: &[&str] = &[
    // Gemini 2.5 family (production)
    "gemini-2.5-pro",
    "gemini-2.5-flash",
    "gemini-2.5-flash-lite",
    // Gemini 2.0 family (deprecated March 2026)
    "gemini-2.0-flash",
    // Gemini 1.5 family (legacy)
    "gemini-1.5-pro",
    "gemini-1.5-flash",
];

/// Default Gemini model (Gemini 2.5 Flash - best speed/cost)
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash";

/// Default API base URL (for API key auth)
const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Code Assist endpoint (for OAuth Bearer token auth — same as Gemini CLI)
const CODE_ASSIST_BASE_URL: &str = "https://cloudcode-pa.googleapis.com";
const CODE_ASSIST_API_VERSION: &str = "v1internal";

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
    /// Error details array (may contain retryDelay for 429 responses)
    #[serde(default)]
    pub details: Option<Vec<serde_json::Value>>,
}

// ============================================================================
// Schema Sanitization
// ============================================================================

/// Fields not supported by Gemini's OpenAPI Schema subset.
/// See: https://ai.google.dev/api/caching#Schema
const UNSUPPORTED_SCHEMA_FIELDS: &[&str] = &["default", "additionalProperties"];

/// Recursively strip JSON Schema fields that Gemini API does not support.
///
/// Gemini accepts only a limited subset of OpenAPI Schema:
/// `type`, `format`, `description`, `nullable`, `enum`, `items`,
/// `properties`, `required`.
/// Sending unsupported fields like `default` or `additionalProperties`
/// causes INVALID_ARGUMENT 400 errors.
fn strip_unsupported_schema_fields(value: &mut serde_json::Value) {
    if let Some(obj) = value.as_object_mut() {
        for field in UNSUPPORTED_SCHEMA_FIELDS {
            obj.remove(*field);
        }
        // Recurse into nested schemas
        for (_, v) in obj.iter_mut() {
            strip_unsupported_schema_fields(v);
        }
    } else if let Some(arr) = value.as_array_mut() {
        for v in arr.iter_mut() {
            strip_unsupported_schema_fields(v);
        }
    }
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// Authentication method for Gemini API
#[derive(Clone)]
pub enum GeminiAuth {
    /// Standard API key (appended as `?key=` in URL)
    ApiKey(String),
    /// OAuth Bearer token from Gemini CLI (Antigravity Pro)
    OAuth(String),
}

/// Gemini provider configuration
#[derive(Clone)]
pub struct GeminiConfig {
    /// Authentication method
    pub auth: GeminiAuth,
    /// Authentication source (for logging)
    pub auth_source: AuthSource,
    /// Base URL
    pub base_url: String,
    /// Default model
    pub default_model: String,
    /// Default max tokens
    pub default_max_tokens: u32,
    /// Request timeout
    pub timeout: Duration,
}

// SECURITY: Custom Debug implementation to mask credentials
impl fmt::Debug for GeminiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let masked_auth = match &self.auth {
            GeminiAuth::ApiKey(key) => format!("ApiKey({})", mask_api_key(key)),
            GeminiAuth::OAuth(token) => format!("OAuth({})", mask_api_key(token)),
        };
        f.debug_struct("GeminiConfig")
            .field("auth", &masked_auth)
            .field("auth_source", &self.auth_source)
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
            auth: GeminiAuth::ApiKey(api_key.into()),
            auth_source: AuthSource::ApiKey,
            base_url: DEFAULT_BASE_URL.to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            default_max_tokens: 8192,
            timeout: Duration::from_secs(60),
        }
    }

    /// Create configuration from environment variables.
    ///
    /// Priority:
    /// 1. `GOOGLE_API_KEY` / `GEMINI_API_KEY` env var → `GeminiAuth::ApiKey`
    /// 2. `~/.cratos/google_oauth.json` (Cratos OAuth) → `GeminiAuth::OAuth`
    /// 3. `~/.gemini/oauth_creds.json` (Gemini CLI) → `GeminiAuth::OAuth`
    pub fn from_env() -> Result<Self> {
        let base_url =
            std::env::var("GEMINI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let default_model =
            std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        // 1. Try explicit API key
        if let Ok(api_key) = std::env::var("GOOGLE_API_KEY")
            .or_else(|_| std::env::var("GEMINI_API_KEY"))
        {
            return Ok(Self {
                auth: GeminiAuth::ApiKey(api_key),
                auth_source: AuthSource::ApiKey,
                base_url,
                default_model,
                default_max_tokens: 8192,
                timeout: Duration::from_secs(60),
            });
        }

        // 2. Try Cratos OAuth token
        if let Some(tokens) = cli_auth::read_cratos_google_oauth() {
            if cli_auth::is_token_expired(tokens.expiry_date) {
                info!("Cratos Google OAuth token expired, will attempt refresh on first request");
            }
            // Determine the source of the token based on configuration.
            // If CRATOS_GOOGLE_CLIENT_ID is set AND distinct from the default (restricted) ID, 
            // we assume a custom client ID (standard API).
            // Otherwise, we assume the token was obtained using the default/extracted Gemini CLI ID (Code Assist API).
            let is_custom_client = if let Ok(id) = std::env::var("CRATOS_GOOGLE_CLIENT_ID") {
                id != crate::oauth_config::default_google_client_id()
            } else {
                false
            };
            
            let auth_source = if is_custom_client {
                AuthSource::CratosOAuth
            } else {
                AuthSource::GeminiCli
            };

            return Ok(Self {
                auth: GeminiAuth::OAuth(tokens.access_token),
                auth_source,
                base_url,
                default_model,
                default_max_tokens: 8192,
                timeout: Duration::from_secs(60),
            });
        }

        // 3. Try Gemini CLI OAuth credentials
        if let Some(creds) = cli_auth::read_gemini_oauth() {
            if cli_auth::is_token_expired(creds.expiry_date) {
                info!("Gemini CLI token expired, will attempt refresh on first request");
            }
            return Ok(Self {
                auth: GeminiAuth::OAuth(creds.access_token),
                auth_source: AuthSource::GeminiCli,
                base_url,
                default_model,
                default_max_tokens: 8192,
                timeout: Duration::from_secs(60),
            });
        }

        Err(Error::NotConfigured(
            "GOOGLE_API_KEY or GEMINI_API_KEY not set, and no OAuth credentials found"
                .to_string(),
        ))
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
    /// Lazily resolved Code Assist project ID (for OAuth auth).
    code_assist_project: tokio::sync::OnceCell<String>,
}

// Code Assist API types for OAuth auth (same API as Gemini CLI)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadCodeAssistResponse {
    #[serde(default)]
    cloudaicompanion_project: Option<String>,
    #[serde(default)]
    current_tier: Option<CodeAssistTier>,
    #[serde(default)]
    allowed_tiers: Option<Vec<CodeAssistTier>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodeAssistTier {
    #[allow(dead_code)]
    id: Option<String>,
    #[serde(default)]
    is_default: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OnboardResponse {
    #[serde(default)]
    done: bool,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    response: Option<OnboardResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OnboardResult {
    #[serde(default)]
    cloudaicompanion_project: Option<CloudAiProject>,
}

#[derive(Debug, Deserialize)]
struct CloudAiProject {
    #[serde(default)]
    id: Option<String>,
}

impl GeminiProvider {
    /// Create a new Gemini provider
    pub fn new(config: GeminiConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;

        Ok(Self {
            client,
            config,
            code_assist_project: tokio::sync::OnceCell::new(),
        })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = GeminiConfig::from_env()?;
        Self::new(config)
    }

    /// Resolve Code Assist project ID (called once on first OAuth request).
    async fn resolve_code_assist_project(&self) -> Result<String> {
        let token = match &self.config.auth {
            GeminiAuth::OAuth(t) => t.clone(),
            _ => return Ok(String::new()),
        };

        if let Ok(project) = std::env::var("GOOGLE_CLOUD_PROJECT")
            .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT_ID"))
        {
            return Ok(project);
        }

        let base = format!("{}/{}", CODE_ASSIST_BASE_URL, CODE_ASSIST_API_VERSION);

        let load_body = serde_json::json!({
            "metadata": {
                "ideType": "IDE_UNSPECIFIED",
                "platform": "PLATFORM_UNSPECIFIED",
                "pluginType": "GEMINI",
            }
        });

        let resp = self
            .client
            .post(format!("{}:loadCodeAssist", base))
            .header("Authorization", format!("Bearer {}", token))
            .json(&load_body)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !resp.status().is_success() {
            debug!("Code Assist loadCodeAssist failed: HTTP {}", resp.status());
            return Ok(String::new());
        }

        let load: LoadCodeAssistResponse = resp
            .json()
            .await
            .map_err(|e| Error::InvalidResponse(e.to_string()))?;

        if let Some(project) = load.cloudaicompanion_project {
            return Ok(project);
        }

        if load.current_tier.is_some() {
            return Ok(String::new());
        }

        // Onboard user (free tier)
        let tier_id = load
            .allowed_tiers
            .as_ref()
            .and_then(|tiers| tiers.iter().find(|t| t.is_default))
            .and_then(|t| t.id.clone())
            .unwrap_or_else(|| "FREE".to_string());

        let onboard_body = serde_json::json!({
            "tierId": tier_id,
            "metadata": {
                "ideType": "IDE_UNSPECIFIED",
                "platform": "PLATFORM_UNSPECIFIED",
                "pluginType": "GEMINI",
            }
        });

        let onboard_resp = self
            .client
            .post(format!("{}:onboardUser", base))
            .header("Authorization", format!("Bearer {}", token))
            .json(&onboard_body)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !onboard_resp.status().is_success() {
            return Ok(String::new());
        }

        let mut onboard: OnboardResponse = onboard_resp
            .json()
            .await
            .map_err(|e| Error::InvalidResponse(e.to_string()))?;

        // Poll LRO if needed
        if !onboard.done {
            if let Some(ref op_name) = onboard.name.clone() {
                for _ in 0..12 {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    let poll_resp = self
                        .client
                        .get(format!("{}/{}", base, op_name))
                        .header("Authorization", format!("Bearer {}", token))
                        .send()
                        .await
                        .map_err(|e| Error::Network(e.to_string()))?;
                    if poll_resp.status().is_success() {
                        onboard = poll_resp
                            .json()
                            .await
                            .map_err(|e| Error::InvalidResponse(e.to_string()))?;
                        if onboard.done {
                            break;
                        }
                    }
                }
            }
        }

        if let Some(result) = onboard.response {
            if let Some(project) = result.cloudaicompanion_project {
                if let Some(id) = project.id {
                    return Ok(id);
                }
            }
        }

        Ok(String::new())
    }

    /// Get or lazily resolve the Code Assist project ID.
    async fn get_code_assist_project(&self) -> Result<&str> {
        self.code_assist_project
            .get_or_try_init(|| self.resolve_code_assist_project())
            .await
            .map(|s| s.as_str())
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
            .map(|tool| {
                let mut params = tool.parameters.clone();
                strip_unsupported_schema_fields(&mut params);
                FunctionDeclaration {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: params,
                }
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
        // SECURITY: Don't log the full URL (may contain API key)
        debug!("Sending request to Gemini model: {} (auth_source={:?})", model, self.config.auth_source);

        // Only GeminiCli uses Code Assist API; CratosOAuth uses standard API with Bearer token
        let is_code_assist = self.config.auth_source == AuthSource::GeminiCli;

        let mut request_builder = match &self.config.auth {
            GeminiAuth::ApiKey(key) => {
                let url = format!(
                    "{}/models/{}:generateContent?key={}",
                    self.config.base_url, model, key
                );
                self.client.post(&url)
            }
            GeminiAuth::OAuth(token) if is_code_assist => {
                // GeminiCli OAuth uses Code Assist endpoint
                let url = format!(
                    "{}/{}:generateContent",
                    CODE_ASSIST_BASE_URL, CODE_ASSIST_API_VERSION
                );
                self.client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", token))
            }
            GeminiAuth::OAuth(token) => {
                // CratosOAuth uses standard Gemini API with Bearer token
                let url = format!(
                    "{}/models/{}:generateContent",
                    self.config.base_url, model
                );
                self.client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", token))
            }
        };

        request_builder = request_builder.header("content-type", "application/json");

        // Code Assist wraps request: {model, project, request: {...}}
        let response = if is_code_assist {
            let project_id = self.get_code_assist_project().await?;
            // Code Assist API expects model name without "models/" prefix (e.g. "gemini-2.5-flash")
            // The `model` variable usually comes as "gemini-2.5-flash" from constants.
            let wrapped = serde_json::json!({
                "model": model, 
                "project": project_id,
                "request": &request,
            });
            request_builder
                .json(&wrapped)
                .send()
                .await
                .map_err(|e| Error::Network(e.to_string()))?
        } else {
            request_builder
                .json(&request)
                .send()
                .await
                .map_err(|e| Error::Network(e.to_string()))?
        };

        // Gemini doesn't send standard rate limit headers, but capture any present
        crate::quota::global_quota_tracker()
            .update_from_headers("gemini", response.headers())
            .await;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !status.is_success() {
            // Log raw status for debugging (no sensitive data in status code)
            tracing::warn!(
                status = %status,
                "Gemini API error response"
            );
            if let Ok(error) = serde_json::from_str::<GeminiError>(&body) {
                // Log error status/code/message for debugging (no API keys in these fields)
                tracing::warn!(
                    error_status = %error.error.status,
                    error_code = error.error.code,
                    error_message = %error.error.message,
                    "Gemini API error detail"
                );
                if status.as_u16() == 403
                    && error.error.message.contains("insufficient authentication scopes")
                    && self.config.auth_source == AuthSource::CratosOAuth
                {
                    tracing::error!(
                        "OAuth token missing required scopes. Please re-run `cratos init` to re-authenticate."
                    );
                    return Err(Error::Api(
                        "OAuth token missing required scopes. Please re-run `cratos init` to re-authenticate with Google.".to_string(),
                    ));
                }
                if status.as_u16() == 429 {
                    // Parse retryDelay from Gemini error body if present
                    if let Some(details) = error.error.details.as_ref() {
                        for detail in details {
                            if let Some(delay) = detail.get("retryDelay").and_then(|v| v.as_str())
                            {
                                // Gemini uses e.g. "30s" format
                                if let Some(secs_str) = delay.strip_suffix('s') {
                                    if let Ok(secs) = secs_str.parse::<u64>() {
                                        crate::quota::global_quota_tracker()
                                            .update_from_retry_after("gemini", secs)
                                            .await;
                                    }
                                }
                            }
                        }
                    }
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

        // Code Assist wraps response: {"response": {...}, "traceId": "..."}
        if is_code_assist {
            if let Ok(wrapped) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(inner) = wrapped.get("response") {
                    return serde_json::from_value(inner.clone())
                        .map_err(|e| Error::InvalidResponse(format!("{}: {}", e, body)));
                }
            }
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

        match &config.auth {
            GeminiAuth::ApiKey(key) => assert_eq!(key, "test-key"),
            _ => panic!("Expected ApiKey auth"),
        }
        assert_eq!(config.auth_source, AuthSource::ApiKey);
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

    #[test]
    fn test_config_debug_masks_oauth_token() {
        let config = GeminiConfig {
            auth: GeminiAuth::OAuth("ya29.long-oauth-token-1234567890".to_string()),
            auth_source: AuthSource::GeminiCli,
            base_url: DEFAULT_BASE_URL.to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            default_max_tokens: 8192,
            timeout: Duration::from_secs(60),
        };
        let debug_str = format!("{:?}", config);

        assert!(!debug_str.contains("long-oauth-token"));
        assert!(debug_str.contains("OAuth(ya29...7890)"));
    }

    #[test]
    fn test_strip_unsupported_schema_fields() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path",
                    "default": "/tmp"
                },
                "options": {
                    "type": "object",
                    "additionalProperties": true,
                    "properties": {
                        "recursive": {
                            "type": "boolean",
                            "default": false
                        }
                    }
                }
            },
            "required": ["path"],
            "additionalProperties": false
        });

        strip_unsupported_schema_fields(&mut schema);

        let obj = schema.as_object().unwrap();
        // Top-level additionalProperties removed
        assert!(!obj.contains_key("additionalProperties"));
        // Supported fields preserved
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("required"));

        let path_prop = &schema["properties"]["path"];
        assert_eq!(path_prop.get("type").unwrap(), "string");
        assert_eq!(path_prop.get("description").unwrap(), "File path");
        assert!(path_prop.get("default").is_none());

        let options_prop = &schema["properties"]["options"];
        assert!(options_prop.get("additionalProperties").is_none());

        let recursive_prop = &schema["properties"]["options"]["properties"]["recursive"];
        assert_eq!(recursive_prop.get("type").unwrap(), "boolean");
        assert!(recursive_prop.get("default").is_none());
    }

    #[test]
    fn test_convert_tools_strips_unsupported_fields() {
        let tools = vec![ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "count": {
                        "type": "integer",
                        "default": 10
                    }
                }
            }),
        }];

        let gemini_tools = GeminiProvider::convert_tools(&tools);
        let params = &gemini_tools[0].function_declarations[0].parameters;
        // default should be stripped
        assert!(params["properties"]["count"].get("default").is_none());
        // type should remain
        assert_eq!(params["properties"]["count"]["type"], "integer");
    }
}
