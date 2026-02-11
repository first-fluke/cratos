//! CLI Authentication — Read tokens from Gemini CLI and Codex CLI
//!
//! Allows Cratos to reuse existing CLI subscriptions (Antigravity Pro, ChatGPT Pro/Plus)
//! without requiring separate API keys.
//!
//! Supported:
//! - Gemini CLI: `~/.gemini/oauth_creds.json` (OAuth Bearer)
//! - Codex CLI: `~/.codex/auth.json` (OpenAI Bearer token)

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::{debug, warn};

lazy_static::lazy_static! {
    static ref AUTH_SOURCES: RwLock<HashMap<String, AuthSource>> =
        RwLock::new(HashMap::new());
}

// ============================================================================
// Credential Types
// ============================================================================

/// Gemini CLI OAuth credentials (`~/.gemini/oauth_creds.json`)
#[derive(Debug, Deserialize)]
pub struct GeminiOAuthCreds {
    /// OAuth2 access token
    pub access_token: String,
    /// OAuth2 refresh token
    pub refresh_token: String,
    /// Expiry date in milliseconds since epoch
    #[serde(default)]
    pub expiry_date: i64,
}

/// Codex CLI auth credentials (`~/.codex/auth.json`)
///
/// Actual file structure:
/// ```json
/// {
///   "OPENAI_API_KEY": null,
///   "tokens": { "access_token": "eyJ...", "refresh_token": "rt_...", ... },
///   "last_refresh": "2026-02-..."
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct CodexAuthCreds {
    /// Nested tokens object
    pub tokens: CodexTokens,
}

/// Inner tokens from Codex CLI auth
#[derive(Debug, Deserialize)]
pub struct CodexTokens {
    /// OAuth access token (JWT) for OpenAI API
    pub access_token: String,
    /// Refresh token
    #[serde(default)]
    pub refresh_token: Option<String>,
}

/// Source of authentication for logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSource {
    /// Standard API key from environment variable
    ApiKey,
    /// OAuth token from Cratos browser login
    CratosOAuth,
    /// OAuth token from Gemini CLI (Antigravity Pro)
    GeminiCli,
    /// Bearer token from Codex CLI (ChatGPT Pro/Plus)
    CodexCli,
    /// Bearer token from Google Cloud SDK (gcloud)
    GcloudCli,
}

impl std::fmt::Display for AuthSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthSource::ApiKey => write!(f, "API key"),
            AuthSource::CratosOAuth => write!(f, "Cratos OAuth"),
            AuthSource::GeminiCli => write!(f, "OAuth: Gemini CLI"),
            AuthSource::CodexCli => write!(f, "Codex CLI auth"),
            AuthSource::GcloudCli => write!(f, "Google Cloud SDK (gcloud)"),
        }
    }
}

/// Status of a Cratos OAuth token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CratosOAuthStatus {
    /// Token exists and is not expired
    Valid,
    /// Token exists but is expired
    Expired,
    /// Token file not found
    NotFound,
}

// ============================================================================
// File Paths
// ============================================================================

/// Path to Gemini CLI OAuth credentials
fn gemini_oauth_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".gemini").join("oauth_creds.json"))
}

/// Path to Codex CLI auth credentials
fn codex_auth_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex").join("auth.json"))
}

// ============================================================================
// Read Functions
// ============================================================================

/// Read Gemini CLI OAuth credentials from `~/.gemini/oauth_creds.json`.
///
/// Returns `None` if the file doesn't exist or can't be parsed.
pub fn read_gemini_oauth() -> Option<GeminiOAuthCreds> {
    let path = gemini_oauth_path()?;
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            debug!("Gemini CLI credentials not found at {}", path.display());
            return None;
        }
    };

    match serde_json::from_str::<GeminiOAuthCreds>(&content) {
        Ok(creds) if !creds.access_token.is_empty() => {
            debug!("Read Gemini CLI OAuth credentials");
            Some(creds)
        }
        Ok(_) => {
            debug!("Gemini CLI credentials file has empty access_token");
            None
        }
        Err(e) => {
            warn!("Failed to parse Gemini CLI credentials: {}", e);
            None
        }
    }
}

/// Read Codex CLI auth credentials from `~/.codex/auth.json`.
///
/// Returns `None` if the file doesn't exist or can't be parsed.
pub fn read_codex_auth() -> Option<CodexAuthCreds> {
    let path = codex_auth_path()?;
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            debug!("Codex CLI credentials not found at {}", path.display());
            return None;
        }
    };

    match serde_json::from_str::<CodexAuthCreds>(&content) {
        Ok(creds) if !creds.tokens.access_token.is_empty() => {
            debug!("Read Codex CLI auth credentials");
            Some(creds)
        }
        Ok(_) => {
            debug!("Codex CLI credentials file has empty access_token");
            None
        }
        Err(e) => {
            warn!("Failed to parse Codex CLI credentials: {}", e);
            None
        }
    }
}

// ============================================================================
// Cratos OAuth Read Functions
// ============================================================================

/// Read Cratos Google OAuth tokens from `~/.cratos/google_oauth.json`.
pub fn read_cratos_google_oauth() -> Option<crate::oauth::OAuthTokens> {
    crate::oauth::read_tokens(crate::oauth_config::GOOGLE_TOKEN_FILE)
}

/// Read Cratos OpenAI OAuth tokens from `~/.cratos/openai_oauth.json`.
pub fn read_cratos_openai_oauth() -> Option<crate::oauth::OAuthTokens> {
    crate::oauth::read_tokens(crate::oauth_config::OPENAI_TOKEN_FILE)
}

/// Check Cratos Google OAuth token status.
pub fn check_cratos_google_oauth_status() -> CratosOAuthStatus {
    match read_cratos_google_oauth() {
        Some(tokens) => {
            if is_token_expired(tokens.expiry_date) {
                CratosOAuthStatus::Expired
            } else {
                CratosOAuthStatus::Valid
            }
        }
        None => CratosOAuthStatus::NotFound,
    }
}

/// Check Cratos OpenAI OAuth token status.
pub fn check_cratos_openai_oauth_status() -> CratosOAuthStatus {
    match read_cratos_openai_oauth() {
        Some(tokens) => {
            if is_token_expired(tokens.expiry_date) {
                CratosOAuthStatus::Expired
            } else {
                CratosOAuthStatus::Valid
            }
        }
        None => CratosOAuthStatus::NotFound,
    }
}

// ============================================================================
// Token Validation
// ============================================================================

/// Check if an OAuth token is expired (with 5-minute buffer).
///
/// `expiry_date` is milliseconds since Unix epoch.
/// Returns `true` if expired or if `expiry_date` is 0 (unknown).
#[must_use]
pub fn is_token_expired(expiry_date: i64) -> bool {
    if expiry_date == 0 {
        // Unknown expiry — assume valid, will fail at request time
        return false;
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    let buffer_ms = 5 * 60 * 1000; // 5 minutes
    now_ms >= (expiry_date - buffer_ms)
}

/// Check if Gemini CLI credentials exist and are valid (not expired).
pub fn check_gemini_cli_status() -> GeminiCliStatus {
    let path = match gemini_oauth_path() {
        Some(p) => p,
        None => return GeminiCliStatus::NoHomeDir,
    };

    if !path.exists() {
        return GeminiCliStatus::NotFound;
    }

    match read_gemini_oauth() {
        Some(creds) => {
            if is_token_expired(creds.expiry_date) {
                GeminiCliStatus::Expired
            } else {
                GeminiCliStatus::Valid
            }
        }
        None => GeminiCliStatus::ParseError,
    }
}

/// Check if Codex CLI credentials exist and are readable.
pub fn check_codex_cli_status() -> CodexCliStatus {
    let path = match codex_auth_path() {
        Some(p) => p,
        None => return CodexCliStatus::NoHomeDir,
    };

    if !path.exists() {
        return CodexCliStatus::NotFound;
    }

    match read_codex_auth() {
        Some(_) => CodexCliStatus::Valid,
        None => CodexCliStatus::ParseError,
    }
}

/// Gemini CLI credential status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiCliStatus {
    /// Credentials found and not expired
    Valid,
    /// Credentials found but expired
    Expired,
    /// Credential file not found
    NotFound,
    /// File exists but couldn't be parsed
    ParseError,
    /// Home directory not found
    NoHomeDir,
}

impl std::fmt::Display for GeminiCliStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeminiCliStatus::Valid => write!(f, "valid"),
            GeminiCliStatus::Expired => write!(f, "expired (run `gemini auth login`)"),
            GeminiCliStatus::NotFound => write!(f, "not found"),
            GeminiCliStatus::ParseError => write!(f, "parse error"),
            GeminiCliStatus::NoHomeDir => write!(f, "home dir not found"),
        }
    }
}

/// Codex CLI credential status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexCliStatus {
    /// Credentials found and readable
    Valid,
    /// Credential file not found
    NotFound,
    /// File exists but couldn't be parsed
    ParseError,
    /// Home directory not found
    NoHomeDir,
}

impl std::fmt::Display for CodexCliStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodexCliStatus::Valid => write!(f, "valid"),
            CodexCliStatus::NotFound => write!(f, "not found"),
            CodexCliStatus::ParseError => write!(f, "parse error"),
            CodexCliStatus::NoHomeDir => write!(f, "home dir not found"),
        }
    }
}

// ============================================================================
// Auth Source Registry
// ============================================================================

/// Register the authentication source for a provider.
pub fn register_auth_source(provider: &str, source: AuthSource) {
    if let Ok(mut map) = AUTH_SOURCES.write() {
        map.insert(provider.to_string(), source);
    }
}

/// Get the authentication source for a provider.
pub fn get_auth_source(provider: &str) -> Option<AuthSource> {
    AUTH_SOURCES.read().ok()?.get(provider).copied()
}

/// Get all registered authentication sources.
pub fn get_all_auth_sources() -> HashMap<String, AuthSource> {
    AUTH_SOURCES
        .read()
        .ok()
        .map(|m| m.clone())
        .unwrap_or_default()
}

// ============================================================================
// Token Write (Gemini)
// ============================================================================

/// Write refreshed OAuth credentials back to `~/.gemini/oauth_creds.json`.
///
/// This allows other processes (and future restarts) to pick up the new token
/// without requiring `gemini auth login`.
pub fn write_gemini_oauth(access_token: &str, refresh_token: &str, expiry_date: i64) -> crate::Result<()> {
    let path = gemini_oauth_path().ok_or_else(|| {
        crate::Error::OAuth("Home directory not found".to_string())
    })?;

    let creds = serde_json::json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "expiry_date": expiry_date,
    });

    let json = serde_json::to_string_pretty(&creds)
        .map_err(|e| crate::Error::OAuth(format!("Failed to serialize credentials: {}", e)))?;

    std::fs::write(&path, &json)
        .map_err(|e| crate::Error::OAuth(format!("Failed to write {}: {}", path.display(), e)))?;

    // Set file permissions to 0600 (owner read/write only) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms).ok();
    }

    debug!("Wrote refreshed Gemini OAuth credentials to {}", path.display());
    Ok(())
}

// ============================================================================
// Token Refresh (Gemini)
// ============================================================================

/// Attempt to refresh Gemini CLI token by invoking the CLI.
///
/// Gemini CLI 0.27.3+ does not have an `auth` sub-command.
/// Instead we run a minimal non-interactive query (`-p "hi"`) which
/// triggers the CLI's internal token refresh logic. After the CLI
/// exits, the refreshed credentials are read from disk.
///
/// Returns refreshed credentials, or an error if CLI is not available.
pub async fn refresh_gemini_token() -> crate::Result<GeminiOAuthCreds> {
    use tokio::process::Command;

    debug!("Attempting to refresh Gemini CLI token via minimal query");

    let token_before = read_gemini_oauth().map(|c| c.access_token);

    // Gemini CLI 0.27.3+: `auth` sub-command doesn't exist.
    // A short non-interactive query forces internal token refresh.
    let output = Command::new("gemini")
        .args(["-p", "hi", "-m", "gemini-2.0-flash-lite"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            crate::Error::NotConfigured(format!(
                "Gemini CLI not found. Install: npm i -g @google/gemini-cli ({})",
                e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::NotConfigured(format!(
            "Gemini CLI token refresh failed (exit {}). {}",
            output.status.code().unwrap_or(-1),
            crate::util::truncate_safe(&stderr, 200)
        )));
    }

    // CLI succeeded → it internally refreshed the token → re-read from disk
    let new_creds = read_gemini_oauth().ok_or_else(|| {
        crate::Error::NotConfigured(
            "Gemini CLI ran but credentials not readable".to_string(),
        )
    })?;

    // Verify the token actually changed
    if let Some(ref old) = token_before {
        if *old == new_creds.access_token {
            debug!("Gemini CLI ran but token unchanged — may still be valid");
        } else {
            debug!("Gemini CLI refreshed token successfully (token changed)");
        }
    }

    Ok(new_creds)
}

// ============================================================================
// Google Cloud SDK Integration
// ============================================================================

/// Attempt to get an access token from Google Cloud SDK (`gcloud`).
///
/// Runs `gcloud auth print-access-token`.
pub async fn get_gcloud_access_token() -> crate::Result<String> {
    use tokio::process::Command;

    debug!("Attempting to get token from gcloud CLI");

    let output = Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .await
        .map_err(|e| {
            crate::Error::NotConfigured(format!(
                "gcloud CLI not found. ({})",
                e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::NotConfigured(format!(
            "gcloud token fetch failed. Run `gcloud auth login` or `gcloud auth application-default login`. ({})",
            crate::util::truncate_safe(&stderr, 200)
        )));
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return Err(crate::Error::NotConfigured(
            "gcloud returned empty token".to_string(),
        ));
    }

    Ok(token)
}

/// Blocking version of `get_gcloud_access_token` for use in synchronous configuration loading.
pub fn get_gcloud_access_token_blocking() -> crate::Result<String> {
    use std::process::Command;

    debug!("Attempting to get token from gcloud CLI (blocking)");

    let output = Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .map_err(|e| {
            crate::Error::NotConfigured(format!(
                "gcloud CLI not found. ({})",
                e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::NotConfigured(format!(
            "gcloud token fetch failed. Run `gcloud auth login` or `gcloud auth application-default login`. ({})",
            crate::util::truncate_safe(&stderr, 200)
        )));
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return Err(crate::Error::NotConfigured(
            "gcloud returned empty token".to_string(),
        ));
    }

    Ok(token)
}

/// Get the current Google Cloud project ID (blocking).
pub fn get_gcloud_project_id_blocking() -> crate::Result<String> {
    use std::process::Command;

    debug!("Attempting to get project ID from gcloud CLI (blocking)");

    let output = Command::new("gcloud")
        .args(["config", "get-value", "project"])
        .output()
        .map_err(|e| {
            crate::Error::NotConfigured(format!(
                "gcloud CLI not found. ({})",
                e
            ))
        })?;

    if !output.status.success() {
        return Err(crate::Error::NotConfigured(
            "gcloud project fetch failed".to_string(),
        ));
    }

    let project = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if project.is_empty() {
        return Err(crate::Error::NotConfigured(
            "No project set in gcloud config. Run `gcloud config set project <PROJECT_ID>`".to_string(),
        ));
    }
    
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemini_oauth_json() {
        let json = r#"{
            "access_token": "ya29.test-token-abc",
            "refresh_token": "1//test-refresh",
            "expiry_date": 1800000000000
        }"#;

        let creds: GeminiOAuthCreds = serde_json::from_str(json).unwrap();
        assert_eq!(creds.access_token, "ya29.test-token-abc");
        assert_eq!(creds.refresh_token, "1//test-refresh");
        assert_eq!(creds.expiry_date, 1800000000000);
    }

    #[test]
    fn test_parse_codex_auth_json() {
        let json = r#"{
            "OPENAI_API_KEY": null,
            "tokens": {
                "access_token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.test",
                "refresh_token": "rt_abc123",
                "account_id": "00000000-0000-0000-0000-000000000000"
            },
            "last_refresh": "2026-02-01T00:00:00Z"
        }"#;

        let creds: CodexAuthCreds = serde_json::from_str(json).unwrap();
        assert!(creds.tokens.access_token.starts_with("eyJ"));
        assert_eq!(
            creds.tokens.refresh_token.as_deref(),
            Some("rt_abc123")
        );
    }

    #[test]
    fn test_is_token_expired_future() {
        // Far future — not expired
        let future_ms = chrono::Utc::now().timestamp_millis() + 3_600_000; // +1 hour
        assert!(!is_token_expired(future_ms));
    }

    #[test]
    fn test_is_token_expired_past() {
        // Past — expired
        let past_ms = chrono::Utc::now().timestamp_millis() - 1000;
        assert!(is_token_expired(past_ms));
    }

    #[test]
    fn test_is_token_expired_within_buffer() {
        // Within 5-minute buffer — considered expired
        let almost_ms = chrono::Utc::now().timestamp_millis() + 60_000; // +1 min (< 5 min buffer)
        assert!(is_token_expired(almost_ms));
    }

    #[test]
    fn test_is_token_expired_zero() {
        // Unknown expiry — assume valid
        assert!(!is_token_expired(0));
    }

    #[test]
    fn test_auth_source_display() {
        assert_eq!(AuthSource::ApiKey.to_string(), "API key");
        assert_eq!(AuthSource::GeminiCli.to_string(), "OAuth: Gemini CLI");
        assert_eq!(AuthSource::CodexCli.to_string(), "Codex CLI auth");
    }

    #[test]
    fn test_gemini_cli_status_display() {
        assert_eq!(GeminiCliStatus::Valid.to_string(), "valid");
        assert_eq!(
            GeminiCliStatus::Expired.to_string(),
            "expired (run `gemini auth login`)"
        );
    }
}
