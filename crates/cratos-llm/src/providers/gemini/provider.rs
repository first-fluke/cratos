//! Gemini provider implementation

use super::config::{downgrade_model, GeminiAuth, GeminiConfig, MODELS};
use super::convert::{convert_messages, convert_tool_choice, convert_tools};
use super::security::sanitize_api_error;
use super::types::*;
use crate::cli_auth::{self, AuthSource};
use crate::error::{Error, Result};
use crate::router::{
    CompletionRequest, CompletionResponse, LlmProvider, TokenUsage, ToolCall,
    ToolCompletionRequest, ToolCompletionResponse,
};
use reqwest::Client;
use tracing::{debug, instrument};

/// Google Gemini provider
pub struct GeminiProvider {
    client: Client,
    pub(crate) config: GeminiConfig,
    /// Last retry-after delay reported by Gemini (seconds), used for smart backoff.
    last_retry_after: std::sync::atomic::AtomicU64,
    /// Dynamically refreshed auth token (overrides config.auth when set).
    /// Used to pick up refreshed Gemini CLI tokens without restarting.
    refreshed_auth: std::sync::Mutex<Option<GeminiAuth>>,
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
            last_retry_after: std::sync::atomic::AtomicU64::new(0),
            refreshed_auth: std::sync::Mutex::new(None),
        })
    }

    /// Get the current auth, preferring refreshed over config.
    pub(crate) fn current_auth(&self) -> GeminiAuth {
        self.refreshed_auth
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_else(|| self.config.auth.clone())
    }

    /// Try to refresh/fallback OAuth token. Returns true if refreshed.
    ///
    /// For GeminiCli auth (3-tier fallback):
    /// 1. Re-read from disk (another process may have refreshed) — skip if same token
    /// 2. Direct OAuth refresh using refresh_token + Gemini CLI's client_id/secret
    /// 3. Invoke Gemini CLI with a minimal query to trigger internal token refresh
    ///
    /// For CratosOAuth auth (scope fallback):
    /// If CratosOAuth fails (e.g. insufficient scopes), fall back to Gemini CLI OAuth.
    pub(crate) async fn try_refresh_cli_token(&self) -> bool {
        match self.config.auth_source {
            AuthSource::GoogleAiPro => self.try_refresh_google_pro_token().await,
            AuthSource::GeminiCli => self.try_refresh_cli_token_inner().await,
            AuthSource::CratosOAuth => self.try_cratos_to_cli_fallback().await,
            _ => false,
        }
    }

    /// Try to refresh Google AI Pro OAuth token.
    async fn try_refresh_google_pro_token(&self) -> bool {
        if let Some(tokens) = cli_auth::read_cratos_google_pro_oauth() {
            if !cli_auth::is_token_expired(tokens.expiry_date) && !tokens.access_token.is_empty() {
                // Re-read from disk and valid
                if let Ok(mut guard) = self.refreshed_auth.lock() {
                    *guard = Some(GeminiAuth::OAuth(tokens.access_token));
                }
                return true;
            }

            if let Some(ref rt) = tokens.refresh_token {
                let config = crate::oauth_config::google_pro_oauth_config();
                match crate::oauth::refresh_token(&config, rt).await {
                    Ok(new_tokens) => {
                        let _ = crate::oauth::save_tokens(&config.token_file, &new_tokens);
                        if let Ok(mut guard) = self.refreshed_auth.lock() {
                            *guard = Some(GeminiAuth::OAuth(new_tokens.access_token));
                        }
                        return true;
                    }
                    Err(e) => {
                        tracing::warn!("Google AI Pro OAuth refresh failed: {}", e);
                    }
                }
            }
        }
        false
    }

    /// CratosOAuth → Gemini CLI OAuth fallback.
    /// Used when CratosOAuth token has insufficient scopes for the standard API.
    async fn try_cratos_to_cli_fallback(&self) -> bool {
        tracing::info!("CratosOAuth failed, attempting Gemini CLI OAuth fallback");

        if let Some(creds) = cli_auth::read_gemini_oauth() {
            // Try using the token directly if not expired
            if !cli_auth::is_token_expired(creds.expiry_date) && !creds.access_token.is_empty() {
                tracing::info!("Falling back to valid Gemini CLI OAuth token");
                if let Ok(mut guard) = self.refreshed_auth.lock() {
                    *guard = Some(GeminiAuth::OAuth(creds.access_token));
                }
                return true;
            }

            // Token expired — try refreshing via Gemini CLI's refresh_token
            if !creds.refresh_token.is_empty() {
                match self.refresh_with_token(&creds.refresh_token).await {
                    Ok(new_access_token) => {
                        tracing::info!("Gemini CLI token refreshed (CratosOAuth fallback)");
                        if let Ok(mut guard) = self.refreshed_auth.lock() {
                            *guard = Some(GeminiAuth::OAuth(new_access_token));
                        }
                        return true;
                    }
                    Err(e) => {
                        tracing::warn!("Gemini CLI OAuth refresh failed: {}", e);
                    }
                }
            }
        }

        // Last resort: invoke Gemini CLI
        match cli_auth::refresh_gemini_token().await {
            Ok(new_creds) => {
                tracing::info!("Gemini CLI token refreshed via CLI (CratosOAuth fallback)");
                if let Ok(mut guard) = self.refreshed_auth.lock() {
                    *guard = Some(GeminiAuth::OAuth(new_creds.access_token));
                }
                true
            }
            Err(e) => {
                tracing::error!("CratosOAuth fallback to Gemini CLI failed: {}", e);
                false
            }
        }
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
        let cli_creds = crate::gemini_auth::resolve_gemini_cli_credentials().ok_or_else(|| {
            Error::OAuth("Gemini CLI credentials not found for refresh".to_string())
        })?;

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

        let new_tokens = crate::oauth::refresh_token(&config, refresh_token)
            .await
            .map_err(|e| Error::OAuth(format!("OAuth token refresh failed: {}", e)))?;

        // Save refreshed tokens back to disk for other processes
        cli_auth::write_gemini_oauth(
            &new_tokens.access_token,
            new_tokens.refresh_token.as_deref().unwrap_or(refresh_token),
            new_tokens.expiry_date,
        )
        .map_err(|e| {
            tracing::warn!(
                "Failed to write refreshed Gemini credentials to disk: {}",
                e
            );
            e
        })
        .ok();

        Ok(new_tokens.access_token)
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = GeminiConfig::from_env(None)?;
        Self::new(config)
    }

    /// Send request to Gemini API (with retry on 429 + automatic model downgrade)
    ///
    /// Returns `(GeminiResponse, actual_model_used)` — the model may differ from
    /// the requested one if a 429 triggered an automatic downgrade.
    pub(crate) async fn send_request(
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
                        let gemini_delay = self
                            .last_retry_after
                            .load(std::sync::atomic::Ordering::Relaxed);
                        
                        // If delay is too long, return Error::RateLimit immediately so the caller 
                        // can trigger provider fallback (e.g., to OpenAI).
                        if gemini_delay >= 30 {
                            tracing::warn!(
                                gemini_hint_secs = gemini_delay,
                                "Gemini rate limit retry_after is too long (>=30s), triggering fallback explicitly."
                            );
                            return Err(Error::RateLimit);
                        }

                        let delay_secs = if gemini_delay > 0 {
                            gemini_delay
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
                    Err(Error::RateLimit) => {
                        // Max retries reached, we will now downgrade.
                        // But if delay is still huge, it's better to just error.
                        let gemini_delay = self.last_retry_after.load(std::sync::atomic::Ordering::Relaxed);
                        if gemini_delay >= 30 {
                            return Err(Error::RateLimit);
                        }
                        break;
                    }
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

    /// Single attempt to send request to Gemini API.
    ///
    /// All auth methods (API key, OAuth) route through the standard Gemini API
    /// (`generativelanguage.googleapis.com`). The Code Assist internal API
    /// (`cloudcode-pa.googleapis.com`) is intentionally NOT used — Google bans
    /// accounts that use it from third-party tools (since Jan 2026).
    async fn send_request_once(
        &self,
        model: &str,
        request: &GeminiRequest,
    ) -> Result<GeminiResponse> {
        // SECURITY: Don't log the full URL (may contain API key)
        debug!(
            "Sending request to Gemini model: {} (auth_source={:?})",
            model, self.config.auth_source
        );

        let current_auth = self.current_auth();
        let mut request_builder = match &current_auth {
            GeminiAuth::ApiKey(key) => {
                let url = format!(
                    "{}/models/{}:generateContent?key={}",
                    self.config.base_url, model, key
                );
                self.client.post(&url)
            }
            GeminiAuth::OAuth(token) => {
                let url = format!("{}/models/{}:generateContent", self.config.base_url, model);
                let mut rb = self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", token));

                if let Some(ref project_id) = self.config.project_id {
                    rb = rb.header("x-goog-user-project", project_id);
                }
                rb
            }
        };

        request_builder = request_builder.header("content-type", "application/json");

        let response = request_builder
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

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
            tracing::warn!(status = %status, "Gemini API error response");
            if let Ok(error) = serde_json::from_str::<GeminiError>(&body) {
                // Log error status/code/message for debugging (no API keys in these fields)
                tracing::warn!(
                    error_status = %error.error.status,
                    error_code = error.error.code,
                    error_message = %error.error.message,
                    "Gemini API error detail"
                );
                if status.as_u16() == 403
                    && error
                        .error
                        .message
                        .contains("insufficient authentication scopes")
                {
                    tracing::warn!(
                        auth_source = ?self.config.auth_source,
                        "OAuth token has insufficient scopes — will attempt authentication fallback"
                    );
                    // Return error with "authentication" keyword to trigger
                    // retry+fallback in send_request() (CratosOAuth → Gemini CLI)
                    return Err(Error::Api(
                        "authentication failed: insufficient scopes for Gemini API".to_string(),
                    ));
                }
                if status.as_u16() == 429 {
                    let mut retry_secs: u64 = 0;
                    // Parse retryDelay from Gemini error details if present
                    if let Some(details) = error.error.details.as_ref() {
                        for detail in details {
                            if let Some(delay) = detail.get("retryDelay").and_then(|v| v.as_str()) {
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
                        self.last_retry_after
                            .store(retry_secs, std::sync::atomic::Ordering::Relaxed);
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
                    "HTTP {}",
                    status
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

        let (system_instruction, contents) = convert_messages(&request.messages);

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
                tracing::warn!(
                    "Gemini response empty (MAX_TOKENS). Code Assist API may be limiting output."
                );
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

        let (system_instruction, contents) = convert_messages(&request.request.messages);

        let generation_config = Some(GenerationConfig {
            temperature: request.request.temperature,
            max_output_tokens: request
                .request
                .max_tokens
                .or(Some(self.config.default_max_tokens)),
            stop_sequences: request.request.stop.clone(),
        });

        let tools = convert_tools(&request.tools);
        let tool_config = convert_tool_choice(&request.tool_choice, &request.tools);

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
                tracing::warn!(
                    "Gemini tool response empty (MAX_TOKENS). Code Assist API may be limiting output."
                );
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
