//! Gemini provider configuration

use crate::cli_auth::{self, AuthSource};
use crate::error::{Error, Result};
use crate::util::mask_api_key;
use std::fmt;
use std::time::Duration;
use tracing::info;

/// Default API base URL (for all auth methods — API key and OAuth Bearer)
pub(crate) const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

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
pub(crate) fn downgrade_model(model: &str) -> Option<&'static str> {
    match model {
        "gemini-3-pro-preview" => Some("gemini-3-flash-preview"),
        // NOTE: Do NOT downgrade gemini-3-flash-preview to gemini-2.5-flash.
        // Gemini 3 thinking models return `thoughtSignature` on function calls.
        // Non-thinking models don't. Mixing them in the same conversation causes
        // "missing thought_signature" 400 errors on subsequent turns.
        "gemini-3-flash-preview" => None,
        "gemini-2.5-pro" => Some("gemini-2.5-flash"),
        "gemini-2.5-flash" => Some("gemini-2.5-flash-lite"),
        _ => None,
    }
}

/// Authentication method for Gemini API
#[derive(Clone)]
pub enum GeminiAuth {
    /// Standard API key (appended as `?key=` in URL)
    ApiKey(String),
    /// OAuth Bearer token (Cratos OAuth, Gemini CLI, or gcloud)
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
    /// If `preferred_provider` is provided (e.g., "google_pro" or "google"),
    /// it will prioritize that specific OAuth token if available.
    ///
    /// Priority (default):
    /// 1. `GOOGLE_API_KEY` / `GEMINI_API_KEY` env var → `GeminiAuth::ApiKey`
    /// 2. `~/.cratos/google_pro_oauth.json` (Google AI Pro) → `GeminiAuth::OAuth`
    /// 3. `~/.cratos/google_oauth.json` (Cratos OAuth) → `GeminiAuth::OAuth`
    /// 4. `~/.gemini/oauth_creds.json` (Gemini CLI) → `GeminiAuth::OAuth`
    pub fn from_env(preferred_provider: Option<&str>) -> Result<Self> {
        let base_url =
            std::env::var("GEMINI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let default_model =
            std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        // 1. Try explicit API key
        if let Ok(api_key) =
            std::env::var("GOOGLE_API_KEY").or_else(|_| std::env::var("GEMINI_API_KEY"))
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

        // 2. Try preferred provider first
        if let Some(pref) = preferred_provider {
            match pref {
                "google_pro" => {
                    if let Some(mut tokens) = cli_auth::read_cratos_google_pro_oauth() {
                        if cli_auth::is_token_expired(tokens.expiry_date) {
                            info!("Preferred Google AI Pro OAuth token expired, attempting refresh...");
                            tokens = Self::try_refresh_blocking(tokens);
                        }
                        return Ok(Self {
                            auth: GeminiAuth::OAuth(tokens.access_token),
                            auth_source: AuthSource::GoogleAiPro,
                            base_url,
                            default_model,
                            default_max_tokens: 8192,
                            timeout: Duration::from_secs(60),
                            project_id: None,
                        });
                    }
                }
                "google" | "gemini" => {
                    if let Some(mut tokens) = cli_auth::read_cratos_google_oauth() {
                        if cli_auth::is_token_expired(tokens.expiry_date) {
                            info!("Preferred Google AI OAuth token expired, attempting refresh...");
                            tokens = Self::try_refresh_blocking(tokens);
                        }
                        return Ok(Self {
                            auth: GeminiAuth::OAuth(tokens.access_token),
                            auth_source: AuthSource::CratosOAuth,
                            base_url,
                            default_model,
                            default_max_tokens: 8192,
                            timeout: Duration::from_secs(60),
                            project_id: None,
                        });
                    }
                }
                _ => {}
            }
        }

        // 3. Try Cratos Google AI Pro OAuth token (Default priority)
        if let Some(mut tokens) = cli_auth::read_cratos_google_pro_oauth() {
            if cli_auth::is_token_expired(tokens.expiry_date) {
                info!("Cratos Google AI Pro OAuth token expired, attempting refresh...");
                tokens = Self::try_refresh_blocking(tokens);
            }
            return Ok(Self {
                auth: GeminiAuth::OAuth(tokens.access_token),
                auth_source: AuthSource::GoogleAiPro,
                base_url,
                default_model,
                default_max_tokens: 8192,
                timeout: Duration::from_secs(60),
                project_id: None,
            });
        }

        // 4. Try Cratos Google OAuth token
        if let Some(mut tokens) = cli_auth::read_cratos_google_oauth() {
            if cli_auth::is_token_expired(tokens.expiry_date) {
                info!("Cratos Google OAuth token expired, attempting refresh...");
                tokens = Self::try_refresh_blocking(tokens);
            }
            return Ok(Self {
                auth: GeminiAuth::OAuth(tokens.access_token),
                auth_source: AuthSource::CratosOAuth,
                base_url,
                default_model,
                default_max_tokens: 8192,
                timeout: Duration::from_secs(60),
                project_id: None,
            });
        }

        // 5. Try Gemini CLI OAuth credentials → routed to Standard API (safe)
        if let Some(creds) = cli_auth::read_gemini_oauth() {
            if cli_auth::is_token_expired(creds.expiry_date) {
                info!("Gemini CLI token expired — run `gemini auth login` to refresh");
            }
            tracing::warn!(
                "Gemini CLI OAuth detected — routing to Standard API (safe mode). \
                 For higher quotas: set GEMINI_API_KEY (https://aistudio.google.com/apikey)"
            );
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

        // 6. Try gcloud CLI
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

        let oauth_config = if tokens.provider == crate::oauth_config::GOOGLE_PRO_TOKEN_FILE {
            crate::oauth_config::google_pro_oauth_config()
        } else {
            crate::oauth_config::google_oauth_config()
        };
        let token_file = oauth_config.token_file.clone();

        // Use a blocking reqwest client for sync context
        let result = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;
            rt.block_on(async {
                crate::oauth::refresh_token(&oauth_config, &refresh_tok)
                    .await
                    .ok()
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
