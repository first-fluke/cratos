//! Provider-specific OAuth2 configurations
//!
//! Contains OAuth2 client IDs and endpoints for Google and OpenAI.
//! Google credentials are base64-encoded (same obfuscation as OpenClaw/Gemini CLI).
//! OpenAI is a public client (PKCE only, no secret).

use crate::oauth::OAuthProviderConfig;

/// Token filename for Google OAuth tokens.
pub const GOOGLE_TOKEN_FILE: &str = "google_oauth.json";

/// Token filename for OpenAI OAuth tokens.
pub const OPENAI_TOKEN_FILE: &str = "openai_oauth.json";

// Google Cloud SDK Client ID (Publicly known as 'gcloud' CLI ID)
// "32555940559.apps.googleusercontent.com"
const GOOGLE_CLIENT_ID_BYTES: &[u8] = &[
    0x38, 0x3A, 0x36, 0x7B, 0x21, 0x3B, 0x30, 0x21, 0x3B, 0x3A, 0x36, 0x27, 0x30, 0x26, 0x20, 0x30,
    0x39, 0x32, 0x3A, 0x3A, 0x32, 0x7B, 0x26, 0x25, 0x25, 0x34, 0x7B, 0x6C, 0x60, 0x60, 0x65, 0x61,
    0x6C, 0x60, 0x60, 0x60, 0x67, 0x66,
];

// "notasecret"
const GOOGLE_CLIENT_SECRET_BYTES: &[u8] =
    &[0x21, 0x30, 0x27, 0x36, 0x30, 0x26, 0x34, 0x21, 0x3A, 0x3B];

fn deobfuscate(bytes: &[u8]) -> String {
    let extracted: Vec<u8> = bytes.iter().map(|b| b ^ 0x55).rev().collect();
    String::from_utf8(extracted).expect("Invalid obfuscated credentials")
}

/// Decode the default Google OAuth client ID.
pub fn default_google_client_id() -> String {
    deobfuscate(GOOGLE_CLIENT_ID_BYTES)
}

/// Decode the default Google OAuth client secret.
pub fn default_google_client_secret() -> String {
    deobfuscate(GOOGLE_CLIENT_SECRET_BYTES)
}

/// Build Google OAuth2 configuration.
///
/// Reads `CRATOS_GOOGLE_CLIENT_ID` / `CRATOS_GOOGLE_CLIENT_SECRET` from env.
/// If they match the credentials extracted from Gemini CLI, we treat them as "default/CLI" credentials
/// (which require Code Assist scopes).
/// If no env vars are present, we try to extract from Gemini CLI.
pub fn google_oauth_config() -> OAuthProviderConfig {
    // 1. Env vars (User override)
    let env_id = std::env::var("CRATOS_GOOGLE_CLIENT_ID").ok();
    let env_secret = std::env::var("CRATOS_GOOGLE_CLIENT_SECRET").ok();

    // Try to resolve Gemini CLI credentials for comparison/fallback
    let gemini_creds = crate::gemini_auth::resolve_gemini_cli_credentials();

    let (client_id, client_secret, is_gemini_cli) =
        if let (Some(id), Some(secret)) = (env_id, env_secret) {
            tracing::debug!("Using custom Google OAuth credentials from Environment");
            (id, secret, false)
        } else if let Some(creds) = gemini_creds {
            tracing::info!("Using Google OAuth credentials from Gemini CLI installation");
            (creds.client_id, creds.client_secret, true)
        } else {
            let default_id = default_google_client_id();
            tracing::info!(
                "Using default Google OAuth credentials (Google Cloud SDK). ID: {}",
                default_id
            );
            (default_id, default_google_client_secret(), true)
        };

    // Scopes for internal/CLI IDs (Gemini CLI, gcloud SDK).
    // Include `generative-language` for Standard API compatibility.
    let restricted_scopes = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/generative-language https://www.googleapis.com/auth/userinfo.email";

    // Standard scopes for Custom Client IDs (User created).
    let standard_scopes = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/generative-language https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";

    let scopes = if is_gemini_cli {
        restricted_scopes.to_string()
    } else {
        standard_scopes.to_string()
    };

    OAuthProviderConfig {
        client_id,
        client_secret,
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        token_url: "https://oauth2.googleapis.com/token".to_string(),
        scopes,
        redirect_path: "/oauth2callback".to_string(),
        extra_auth_params: vec![
            ("access_type".to_string(), "offline".to_string()),
            ("prompt".to_string(), "consent".to_string()),
        ],
        token_file: GOOGLE_TOKEN_FILE.to_string(),
    }
}

/// Build OpenAI OAuth2 configuration.
///
/// Uses the same client_id as Codex CLI (public client, PKCE only, no secret).
pub fn openai_oauth_config() -> OAuthProviderConfig {
    let client_id = std::env::var("CRATOS_OPENAI_CLIENT_ID")
        .unwrap_or_else(|_| "app_EMoamEEZ73f0CkXaXp7hrann".to_string());

    OAuthProviderConfig {
        client_id,
        client_secret: String::new(), // Public client — PKCE only
        auth_url: "https://auth.openai.com/oauth/authorize".to_string(),
        token_url: "https://auth.openai.com/oauth/token".to_string(),
        scopes: "openid profile email offline_access".to_string(),
        redirect_path: "/auth/callback".to_string(),
        extra_auth_params: vec![
            ("id_token_add_organizations".to_string(), "true".to_string()),
            ("codex_cli_simplified_flow".to_string(), "true".to_string()),
        ],
        token_file: OPENAI_TOKEN_FILE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b64_decode() {
        let id = default_google_client_id();
        assert!(id.contains("apps.googleusercontent.com"));
        let secret = default_google_client_secret();
        // gcloud SDK secret is "notasecret"
        assert_eq!(secret, "notasecret");
    }

    #[test]
    fn test_google_config() {
        let cfg = google_oauth_config();
        assert!(cfg.client_id.contains("apps.googleusercontent.com"));
        assert!(!cfg.client_secret.is_empty());
        // Default credentials (gcloud SDK / Gemini CLI) use restricted scopes
        assert!(cfg.scopes.contains("cloud-platform"));
        assert_eq!(cfg.redirect_path, "/oauth2callback");
        assert_eq!(cfg.token_file, "google_oauth.json");
    }

    #[test]
    fn test_openai_config() {
        let cfg = openai_oauth_config();
        assert!(cfg.client_id.starts_with("app_"));
        assert!(cfg.client_secret.is_empty());
        assert_eq!(cfg.redirect_path, "/auth/callback");
        assert_eq!(cfg.token_file, "openai_oauth.json");
    }
}
