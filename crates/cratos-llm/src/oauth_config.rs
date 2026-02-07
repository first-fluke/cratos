//! Provider-specific OAuth2 configurations
//!
//! Contains OAuth2 client IDs and endpoints for Google and OpenAI.
//! These are "desktop app" client IDs which are considered public
//! (the client_secret is embedded in the app binary, same as Gemini CLI / Codex CLI).

use crate::oauth::OAuthProviderConfig;

/// Token filename for Google OAuth tokens.
pub const GOOGLE_TOKEN_FILE: &str = "google_oauth.json";

/// Token filename for OpenAI OAuth tokens.
pub const OPENAI_TOKEN_FILE: &str = "openai_oauth.json";

/// Build Google OAuth2 configuration.
///
/// Uses the same client_id as Gemini CLI (desktop app, public secret).
/// Can be overridden with `CRATOS_GOOGLE_CLIENT_ID` / `CRATOS_GOOGLE_CLIENT_SECRET`.
pub fn google_oauth_config() -> OAuthProviderConfig {
    let client_id = std::env::var("CRATOS_GOOGLE_CLIENT_ID").unwrap_or_else(|_| {
        "".to_string() // Placeholder: ID must be provided via env or extracted from CLI
    });
    let client_secret = std::env::var("CRATOS_GOOGLE_CLIENT_SECRET")
        .unwrap_or_default();

    OAuthProviderConfig {
        client_id,
        client_secret,
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        token_url: "https://oauth2.googleapis.com/token".to_string(),
        scopes: "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/generative-language.retriever https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile".to_string(),
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
/// Uses the same client_id as Codex CLI (public client, no secret).
/// Can be overridden with `CRATOS_OPENAI_CLIENT_ID`.
pub fn openai_oauth_config() -> OAuthProviderConfig {
    let client_id = std::env::var("CRATOS_OPENAI_CLIENT_ID")
        .unwrap_or_else(|_| "app_EMoamEEZ73f0CkXaXp7hrann".to_string());

    OAuthProviderConfig {
        client_id,
        client_secret: String::new(), // Public client — no secret
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
    fn test_google_config() {
        let cfg = google_oauth_config();
        assert!(cfg.client_id.contains("apps.googleusercontent.com"));
        assert!(cfg.client_secret.is_empty());
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
