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
/// Pricing per 1M tokens (approximate):
/// - gemini-2.5-flash-lite: ~$0.10 (cheapest)
/// - gemini-2.5-flash: $0.075/$0.60
/// - gemini-3-flash-preview: preview pricing
/// - gemini-2.5-pro: $1.25/$15.00
/// - gemini-3-pro-preview: preview pricing
pub const MODELS: &[&str] = &[
    // Gemini 3 family (preview)
    "gemini-3-flash-preview",
    "gemini-3-pro-preview",
    // Gemini 2.5 family (stable)
    "gemini-2.5-pro",
    "gemini-2.5-flash",
    "gemini-2.5-flash-lite",
    // Gemini 2.0 family (deprecated March 2026)
    "gemini-2.0-flash",
];

/// Default Gemini model
pub const DEFAULT_MODEL: &str = "gemini-3-flash-preview";

/// 429 시 한 단계 저렴한 모델로 다운그레이드
fn downgrade_model(model: &str) -> Option<&'static str> {
    match model {
        "gemini-3-pro-preview" => Some("gemini-3-flash-preview"),
        "gemini-3-flash-preview" => Some("gemini-2.5-flash"),
        "gemini-2.5-pro" => Some("gemini-2.5-flash"),
        "gemini-2.5-flash" => Some("gemini-2.5-flash-lite"),
        _ => None,
    }
}

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
    #[serde(default)]
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
        /// Gemini 3+ thought signature — must be preserved for multi-turn conversations
        #[serde(
            rename = "thoughtSignature",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        thought_signature: Option<String>,
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
    /// May be absent for empty/thinking-only responses
    #[serde(default)]
    candidates_token_count: Option<u32>,
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
    /// Google Cloud Project ID (optional, for GcloudCli)
    pub project_id: Option<String>,
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
            .field("project_id", &self.project_id)
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
            project_id: None,
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
                project_id: None,
            });
        }

        // 2. Try Cratos OAuth token
        if let Some(mut tokens) = cli_auth::read_cratos_google_oauth() {
            if cli_auth::is_token_expired(tokens.expiry_date) {
                info!("Cratos Google OAuth token expired, attempting refresh...");
                tokens = Self::try_refresh_blocking(tokens);
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
                project_id: None,
            });
        }

        // 3. Try Gemini CLI OAuth credentials
        if let Some(creds) = cli_auth::read_gemini_oauth() {
            if cli_auth::is_token_expired(creds.expiry_date) {
                info!("Gemini CLI token expired — run `gemini auth login` to refresh");
            }
            return Ok(Self {
                auth: GeminiAuth::OAuth(creds.access_token),
                auth_source: AuthSource::GeminiCli,
                base_url,
                default_model,
                default_max_tokens: 8192,
                timeout: Duration::from_secs(60),
                project_id: None,
            });
        }

        // 4. Try gcloud CLI
        if let Ok(token) = cli_auth::get_gcloud_access_token_blocking() {
            info!("Using Google Cloud SDK (gcloud) credentials");
            let project_id = cli_auth::get_gcloud_project_id_blocking().ok();
            if let Some(ref p) = project_id {
                info!("Using gcloud project: {}", p);
            }
            return Ok(Self {
                auth: GeminiAuth::OAuth(token),
                auth_source: AuthSource::GcloudCli,
                base_url,
                default_model,
                default_max_tokens: 8192,
                timeout: Duration::from_secs(60),
                project_id,
            });
        }

        Err(Error::NotConfigured(
            "GOOGLE_API_KEY, GEMINI_API_KEY, Cratos OAuth, Gemini OAuth, or gcloud credentials not found"
                .to_string(),
        ))
    }

    /// Try to refresh an expired OAuth token synchronously (blocking).
    /// Returns the refreshed tokens on success, or the original tokens on failure.
    fn try_refresh_blocking(tokens: crate::oauth::OAuthTokens) -> crate::oauth::OAuthTokens {
        let refresh_tok = match tokens.refresh_token.as_deref() {
            Some(rt) if !rt.is_empty() => rt.to_string(),
            _ => {
                tracing::warn!("No refresh token available, using expired access token");
                return tokens;
            }
        };

        let oauth_config = crate::oauth_config::google_oauth_config();
        let token_file = oauth_config.token_file.clone();

        // Use a blocking reqwest client for sync context
        let result = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;
            rt.block_on(async {
                crate::oauth::refresh_token(&oauth_config, &refresh_tok).await.ok()
            })
        })
        .join()
        .ok()
        .flatten();

        match result {
            Some(new_tokens) => {
                if let Err(e) = crate::oauth::save_tokens(&token_file, &new_tokens) {
                    tracing::warn!("Failed to save refreshed tokens: {}", e);
                } else {
                    info!("Google OAuth token refreshed successfully");
                }
                new_tokens
            }
            None => {
                tracing::warn!("Failed to refresh Google OAuth token, using expired token");
                tokens
            }
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

/// Google Gemini provider
pub struct GeminiProvider {
    client: Client,
    config: GeminiConfig,
    /// Lazily resolved Code Assist project ID (for OAuth auth).
    /// Uses RwLock instead of OnceCell so it can be reset after token refresh.
    code_assist_project: tokio::sync::RwLock<Option<String>>,
    /// Last retry-after delay reported by Gemini (seconds), used for smart backoff.
    last_retry_after: std::sync::atomic::AtomicU64,
    /// Dynamically refreshed auth token (overrides config.auth when set).
    /// Used to pick up refreshed Gemini CLI tokens without restarting.
    refreshed_auth: std::sync::Mutex<Option<GeminiAuth>>,
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
            code_assist_project: tokio::sync::RwLock::new(None),
            last_retry_after: std::sync::atomic::AtomicU64::new(0),
            refreshed_auth: std::sync::Mutex::new(None),
        })
    }

    /// Get the current auth, preferring refreshed over config.
    fn current_auth(&self) -> GeminiAuth {
        self.refreshed_auth
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_else(|| self.config.auth.clone())
    }

    /// Try to refresh Gemini CLI OAuth token. Returns true if refreshed.
    ///
    /// Strategy (3-tier fallback):
    /// 1. Re-read from disk (another process may have refreshed) — skip if same token
    /// 2. Direct OAuth refresh using refresh_token + Gemini CLI's client_id/secret
    /// 3. Invoke Gemini CLI with a minimal query to trigger internal token refresh
    ///
    /// On success, also invalidates the Code Assist project cache so the next
    /// request calls `loadCodeAssist` with the new token.
    async fn try_refresh_cli_token(&self) -> bool {
        if self.config.auth_source != AuthSource::GeminiCli {
            return false;
        }

        let refreshed = self.try_refresh_cli_token_inner().await;
        if refreshed {
            // Invalidate Code Assist project cache → next request will
            // call loadCodeAssist with the refreshed token.
            *self.code_assist_project.write().await = None;
            tracing::debug!("Code Assist project cache invalidated after token refresh");
        }
        refreshed
    }

    /// Inner implementation of CLI token refresh (3-tier).
    async fn try_refresh_cli_token_inner(&self) -> bool {
        // Tier 1: Re-read from disk — maybe another process already refreshed
        if let Some(creds) = cli_auth::read_gemini_oauth() {
            if !cli_auth::is_token_expired(creds.expiry_date) && !creds.access_token.is_empty() {
                // Check if disk token is the same as the current one
                let is_same = matches!(self.current_auth(), GeminiAuth::OAuth(ref t) if t == &creds.access_token);
                if is_same {
                    tracing::debug!("Disk token same as current, skipping to Tier 2");
                } else {
                    tracing::info!("Gemini CLI token re-read from disk (different, valid)");
                    if let Ok(mut guard) = self.refreshed_auth.lock() {
                        *guard = Some(GeminiAuth::OAuth(creds.access_token));
                    }
                    return true;
                }
            }

            // Tier 2: Token expired or same — try direct OAuth refresh
            if !creds.refresh_token.is_empty() {
                match self.refresh_with_token(&creds.refresh_token).await {
                    Ok(new_access_token) => {
                        tracing::info!("Gemini CLI token refreshed via OAuth");
                        if let Ok(mut guard) = self.refreshed_auth.lock() {
                            *guard = Some(GeminiAuth::OAuth(new_access_token));
                        }
                        return true;
                    }
                    Err(e) => {
                        tracing::warn!("Direct OAuth refresh failed: {}", e);
                    }
                }
            }
        }

        // Tier 3: Fallback — invoke Gemini CLI
        match cli_auth::refresh_gemini_token().await {
            Ok(new_creds) => {
                tracing::info!("Gemini CLI token refreshed via CLI invocation");
                if let Ok(mut guard) = self.refreshed_auth.lock() {
                    *guard = Some(GeminiAuth::OAuth(new_creds.access_token));
                }
                true
            }
            Err(e) => {
                tracing::error!("All Gemini token refresh methods failed: {}", e);
                false
            }
        }
    }

    /// Refresh access token using the refresh_token and Gemini CLI's OAuth credentials.
    ///
    /// Resolves client_id/secret from the local Gemini CLI installation, then calls
    /// Google's OAuth token endpoint directly.
    async fn refresh_with_token(&self, refresh_token: &str) -> Result<String> {
        // Get Gemini CLI's OAuth client_id/secret
        let cli_creds = crate::gemini_auth::resolve_gemini_cli_credentials()
            .ok_or_else(|| Error::OAuth("Gemini CLI credentials not found for refresh".to_string()))?;

        let config = crate::oauth::OAuthProviderConfig {
            client_id: cli_creds.client_id,
            client_secret: cli_creds.client_secret,
            auth_url: String::new(), // not needed for refresh
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            scopes: String::new(), // not needed for refresh
            redirect_path: String::new(),
            extra_auth_params: vec![],
            token_file: String::new(),
        };

        let new_tokens = crate::oauth::refresh_token(&config, refresh_token).await
            .map_err(|e| Error::OAuth(format!("OAuth token refresh failed: {}", e)))?;

        // Save refreshed tokens back to disk for other processes
        cli_auth::write_gemini_oauth(
            &new_tokens.access_token,
            new_tokens.refresh_token.as_deref().unwrap_or(refresh_token),
            new_tokens.expiry_date,
        ).map_err(|e| {
            tracing::warn!("Failed to write refreshed Gemini credentials to disk: {}", e);
            e
        }).ok();

        Ok(new_tokens.access_token)
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = GeminiConfig::from_env()?;
        Self::new(config)
    }

    /// Resolve Code Assist project ID.
    ///
    /// Uses `current_auth()` (which includes refreshed tokens) instead of
    /// `config.auth` so that after token refresh the new token is used
    /// for the `loadCodeAssist` call that activates the Code Assist session.
    async fn resolve_code_assist_project(&self) -> Result<String> {
        let token = match self.current_auth() {
            GeminiAuth::OAuth(t) => t,
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

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            debug!("Code Assist loadCodeAssist failed: HTTP {} body={}", status, body);
            return Ok(String::new());
        }

        let body_text = resp.text().await.map_err(|e| Error::InvalidResponse(e.to_string()))?;
        debug!("Code Assist loadCodeAssist response: {}", &body_text[..body_text.len().min(500)]);

        let load: LoadCodeAssistResponse = serde_json::from_str(&body_text)
            .map_err(|e| Error::InvalidResponse(format!("{}: {}", e, &body_text[..body_text.len().min(200)])))?;

        if let Some(ref project) = load.cloudaicompanion_project {
            debug!("Code Assist project resolved: {}", project);
            return Ok(project.clone());
        }

        if load.current_tier.is_some() {
            debug!("Code Assist: has current_tier but no project, returning empty");
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
    ///
    /// Returns a cached value if available; otherwise resolves and caches.
    /// The cache is invalidated after token refresh so `loadCodeAssist`
    /// is called again with the new token.
    async fn get_code_assist_project(&self) -> Result<String> {
        // Fast path: return cached value
        if let Some(ref project) = *self.code_assist_project.read().await {
            return Ok(project.clone());
        }
        // Slow path: resolve and cache
        let project = self.resolve_code_assist_project().await?;
        *self.code_assist_project.write().await = Some(project.clone());
        Ok(project)
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
                    let mut parts: Vec<GeminiPart> = Vec::new();
                    if !msg.content.is_empty() {
                        parts.push(GeminiPart::Text {
                            text: msg.content.clone(),
                        });
                    }
                    // Include function calls from assistant's tool_calls
                    for tc in &msg.tool_calls {
                        let args = serde_json::from_str(&tc.arguments)
                            .unwrap_or_else(|_| serde_json::json!({}));
                        parts.push(GeminiPart::FunctionCall {
                            function_call: FunctionCall {
                                name: tc.name.clone(),
                                args,
                            },
                            // Gemini 3+ thought signatures must be preserved exactly
                            thought_signature: tc.thought_signature.clone(),
                        });
                    }
                    if !parts.is_empty() {
                        gemini_contents.push(GeminiContent {
                            role: Some("model".to_string()),
                            parts,
                        });
                    }
                }
                MessageRole::Tool => {
                    if let Some(tool_name) = &msg.name {
                        // Parse the content as JSON for the response
                        let response_value = serde_json::from_str(&msg.content)
                            .unwrap_or_else(|_| serde_json::json!({"result": msg.content}));

                        let part = GeminiPart::FunctionResponse {
                            function_response: FunctionResponse {
                                name: tool_name.clone(),
                                response: response_value,
                            },
                        };

                        // Gemini requires all FunctionResponse parts in a single user turn
                        // matching the number of FunctionCall parts. Merge consecutive
                        // Tool messages into one GeminiContent.
                        if let Some(last) = gemini_contents.last_mut() {
                            if last.role.as_deref() == Some("user")
                                && last.parts.iter().all(|p| matches!(p, GeminiPart::FunctionResponse { .. }))
                            {
                                last.parts.push(part);
                            } else {
                                gemini_contents.push(GeminiContent {
                                    role: Some("user".to_string()),
                                    parts: vec![part],
                                });
                            }
                        } else {
                            gemini_contents.push(GeminiContent {
                                role: Some("user".to_string()),
                                parts: vec![part],
                            });
                        }
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

    /// Send request to Gemini API (with retry on 429 + automatic model downgrade)
    ///
    /// Returns `(GeminiResponse, actual_model_used)` — the model may differ from
    /// the requested one if a 429 triggered an automatic downgrade.
    async fn send_request(
        &self,
        model: &str,
        request: GeminiRequest,
    ) -> Result<(GeminiResponse, String)> {
        const MAX_RETRIES: u32 = 2;
        let mut current_model = model.to_string();

        loop {
            for attempt in 0..=MAX_RETRIES {
                match self.send_request_once(&current_model, &request).await {
                    Ok(resp) => return Ok((resp, current_model)),
                    Err(Error::RateLimit) if attempt < MAX_RETRIES => {
                        let gemini_delay =
                            self.last_retry_after.load(std::sync::atomic::Ordering::Relaxed);
                        let delay_secs = if gemini_delay > 0 {
                            gemini_delay.clamp(1, 15)
                        } else {
                            2 + (attempt as u64) * 2 // exponential: 2, 4
                        };
                        tracing::info!(
                            attempt = attempt + 1,
                            model = %current_model,
                            delay_secs,
                            gemini_hint_secs = gemini_delay,
                            "Gemini rate limited, retrying same model"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                    }
                    Err(Error::RateLimit) => break, // MAX_RETRIES exhausted, try downgrade
                    Err(Error::ServerError(ref msg)) if attempt < MAX_RETRIES => {
                        let delay_secs = 2 + (attempt as u64) * 3; // 2, 5
                        tracing::warn!(
                            attempt = attempt + 1,
                            model = %current_model,
                            delay_secs,
                            error = %msg,
                            "Gemini server error, retrying"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                    }
                    Err(Error::ServerError(msg)) => {
                        // MAX_RETRIES exhausted — propagate as ServerError for fallback
                        return Err(Error::ServerError(msg));
                    }
                    Err(Error::Api(ref msg)) if msg.contains("authentication") && attempt == 0 => {
                        // Token expired — try refreshing (disk re-read → OAuth refresh → CLI)
                        if self.try_refresh_cli_token().await {
                            tracing::info!("Retrying after token refresh");
                            continue;
                        }
                        // Token refresh failed — provide actionable guidance for GeminiCli users
                        if self.config.auth_source == AuthSource::GeminiCli {
                            tracing::error!(
                                "Gemini CLI token refresh failed. Consider: \
                                 1) Set GEMINI_API_KEY for direct API access, \
                                 2) Run `gemini` CLI to refresh token, \
                                 3) Configure a fallback provider in config."
                            );
                        }
                        return Err(Error::Api(msg.clone()));
                    }
                    Err(e) => return Err(e),
                }
            }

            // Same-model retries exhausted → try downgrading
            if let Some(cheaper) = downgrade_model(&current_model) {
                tracing::warn!(
                    from = %current_model,
                    to = %cheaper,
                    "Gemini rate limited, downgrading model"
                );
                current_model = cheaper.to_string();
            } else {
                return Err(Error::RateLimit);
            }
        }
    }

    /// Single attempt to send request to Gemini API
    async fn send_request_once(&self, model: &str, request: &GeminiRequest) -> Result<GeminiResponse> {
        // SECURITY: Don't log the full URL (may contain API key)
        debug!("Sending request to Gemini model: {} (auth_source={:?})", model, self.config.auth_source);

        // Only GeminiCli auth uses Code Assist API; all others use standard API
        let is_code_assist = self.config.auth_source == AuthSource::GeminiCli;

        let current_auth = self.current_auth();
        let mut request_builder = match &current_auth {
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
                // GcloudCli uses standard API but helps to identify project for quota
                let url = format!(
                    "{}/models/{}:generateContent",
                    self.config.base_url, model
                );
                let mut rb = self.client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", token));
                
                if self.config.auth_source == AuthSource::GcloudCli {
                    if let Some(ref project_id) = self.config.project_id {
                        rb = rb.header("x-goog-user-project", project_id);
                    }
                }
                rb
            }
        };

        request_builder = request_builder.header("content-type", "application/json");

        // Code Assist wraps request: {model, project, request: {...}}
        let response = if is_code_assist {
            let project_id = self.get_code_assist_project().await?;
            debug!("Code Assist: model={}, project_id={:?}", model, if project_id.is_empty() { "<EMPTY>" } else { project_id.as_str() });
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
                    let mut retry_secs: u64 = 0;
                    // Parse retryDelay from Gemini error details if present
                    if let Some(details) = error.error.details.as_ref() {
                        for detail in details {
                            if let Some(delay) = detail.get("retryDelay").and_then(|v| v.as_str())
                            {
                                if let Some(secs_str) = delay.strip_suffix('s') {
                                    if let Ok(secs) = secs_str.parse::<u64>() {
                                        retry_secs = secs;
                                    }
                                }
                            }
                        }
                    }
                    // Also parse from message: "Your quota will reset after Xs."
                    if retry_secs == 0 {
                        if let Some(after_pos) = error.error.message.find("reset after ") {
                            let rest = &error.error.message[after_pos + 12..];
                            if let Some(s_pos) = rest.find('s') {
                                if let Ok(secs) = rest[..s_pos].trim().parse::<u64>() {
                                    retry_secs = secs;
                                }
                            }
                        }
                    }
                    if retry_secs > 0 {
                        self.last_retry_after.store(retry_secs, std::sync::atomic::Ordering::Relaxed);
                        crate::quota::global_quota_tracker()
                            .update_from_retry_after("gemini", retry_secs)
                            .await;
                    }
                    return Err(Error::RateLimit);
                }
                // 5xx server errors — retryable
                if status.is_server_error() {
                    return Err(Error::ServerError(sanitize_api_error(&format!(
                        "{}: {}",
                        error.error.status, error.error.message
                    ))));
                }
                // SECURITY: Sanitize error messages
                return Err(Error::Api(sanitize_api_error(&format!(
                    "{}: {}",
                    error.error.status, error.error.message
                ))));
            }
            // 5xx without parseable error body — still retryable
            if status.is_server_error() {
                return Err(Error::ServerError(sanitize_api_error(&format!(
                    "HTTP {}", status
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

        let (response, actual_model) = self.send_request(model, gemini_request).await?;

        let candidate = response
            .candidates
            .first()
            .ok_or_else(|| Error::InvalidResponse("No candidates in response".to_string()))?;

        // Extract text content
        let mut content: String = candidate
            .content
            .parts
            .iter()
            .filter_map(|part| match part {
                GeminiPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        if content.is_empty() {
            if candidate.finish_reason.as_deref() == Some("MAX_TOKENS") {
                tracing::warn!("Gemini response empty (MAX_TOKENS). Code Assist API may be limiting output.");
            }
            content = "(empty response)".to_string();
        }

        let usage = response.usage_metadata.map(|u| TokenUsage {
            prompt_tokens: u.prompt_token_count,
            completion_tokens: u.candidates_token_count.unwrap_or(0),
            total_tokens: u.total_token_count,
        });

        Ok(CompletionResponse {
            content,
            usage,
            finish_reason: candidate.finish_reason.clone(),
            model: actual_model,
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

        let (response, actual_model) = self.send_request(model, gemini_request).await?;

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
                GeminiPart::FunctionCall {
                    function_call,
                    thought_signature,
                } => {
                    tool_calls.push(ToolCall {
                        id: uuid::Uuid::new_v4().to_string(), // Gemini doesn't provide IDs
                        name: function_call.name.clone(),
                        arguments: serde_json::to_string(&function_call.args)
                            .unwrap_or_else(|_| "{}".to_string()),
                        thought_signature: thought_signature.clone(),
                    });
                }
                _ => {}
            }
        }

        if content.is_none() && tool_calls.is_empty() {
            if candidate.finish_reason.as_deref() == Some("MAX_TOKENS") {
                tracing::warn!("Gemini tool response empty (MAX_TOKENS). Code Assist API may be limiting output.");
            }
            content = Some("(empty response)".to_string());
        }

        let usage = response.usage_metadata.map(|u| TokenUsage {
            prompt_tokens: u.prompt_token_count,
            completion_tokens: u.candidates_token_count.unwrap_or(0),
            total_tokens: u.total_token_count,
        });

        Ok(ToolCompletionResponse {
            content,
            tool_calls,
            usage,
            finish_reason: candidate.finish_reason.clone(),
            model: actual_model,
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
        assert!(MODELS.contains(&"gemini-3-flash-preview"));
        assert!(MODELS.contains(&"gemini-2.5-flash"));
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
            project_id: None,
        };
        let debug_str = format!("{:?}", config);

        assert!(!debug_str.contains("long-oauth-token"));
        assert!(debug_str.contains("OAuth(ya29...7890)"));
    }

    #[test]
    fn test_downgrade_chain() {
        assert_eq!(downgrade_model("gemini-3-pro-preview"), Some("gemini-3-flash-preview"));
        assert_eq!(downgrade_model("gemini-3-flash-preview"), Some("gemini-2.5-flash"));
        assert_eq!(downgrade_model("gemini-2.5-pro"), Some("gemini-2.5-flash"));
        assert_eq!(
            downgrade_model("gemini-2.5-flash"),
            Some("gemini-2.5-flash-lite")
        );
        assert_eq!(downgrade_model("gemini-2.5-flash-lite"), None);
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
