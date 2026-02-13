//! Generic OAuth2 PKCE Engine
//!
//! Provides OAuth2 Authorization Code flow with PKCE (S256)
//! for desktop applications. Handles code generation, token exchange,
//! refresh, and secure token storage with AES-256-GCM encryption.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tracing::{debug, warn};

/// OAuth2 provider configuration (provider-agnostic).
#[derive(Debug, Clone)]
pub struct OAuthProviderConfig {
    /// OAuth2 client ID
    pub client_id: String,
    /// OAuth2 client secret (empty for public clients)
    pub client_secret: String,
    /// Authorization endpoint URL
    pub auth_url: String,
    /// Token endpoint URL
    pub token_url: String,
    /// Space-separated scopes
    pub scopes: String,
    /// Redirect path (e.g., `/oauth2callback`)
    pub redirect_path: String,
    /// Extra query parameters for authorization URL
    pub extra_auth_params: Vec<(String, String)>,
    /// Token filename in `~/.cratos/` (e.g., `google_oauth.json`)
    pub token_file: String,
}

/// PKCE pair: code_verifier + code_challenge (S256).
#[derive(Debug, Clone)]
pub struct PkcePair {
    /// The random verifier (sent during token exchange)
    pub code_verifier: String,
    /// The SHA-256 hash of the verifier (sent during authorization)
    pub code_challenge: String,
}

/// Stored OAuth2 tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    /// Access token for API calls
    pub access_token: String,
    /// Refresh token for obtaining new access tokens
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Expiry date in milliseconds since Unix epoch
    #[serde(default)]
    pub expiry_date: i64,
    /// Provider identifier (e.g., "google", "openai")
    #[serde(default)]
    pub provider: String,
}

/// Generate a PKCE pair (code_verifier + S256 code_challenge).
pub fn generate_pkce() -> PkcePair {
    let mut buf = [0u8; 64];
    getrandom::getrandom(&mut buf).expect("failed to generate random bytes");
    let code_verifier = URL_SAFE_NO_PAD.encode(buf);

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let digest = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(digest);

    PkcePair {
        code_verifier,
        code_challenge,
    }
}

/// Generate a cryptographically random CSRF state parameter.
pub fn generate_state() -> String {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).expect("failed to generate random bytes");
    URL_SAFE_NO_PAD.encode(buf)
}

/// Build the full authorization URL for the user's browser.
pub fn build_auth_url(
    config: &OAuthProviderConfig,
    redirect_uri: &str,
    pkce: &PkcePair,
    state: &str,
) -> String {
    let mut params = vec![
        ("client_id", config.client_id.as_str()),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", config.scopes.as_str()),
        ("code_challenge", pkce.code_challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("state", state),
    ];

    // Collect references to extra params
    let extra_refs: Vec<(&str, &str)> = config
        .extra_auth_params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    params.extend(extra_refs.iter().copied());

    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoded(v)))
        .collect::<Vec<_>>()
        .join("&");

    format!("{}?{}", config.auth_url, query)
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code(
    config: &OAuthProviderConfig,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> crate::Result<OAuthTokens> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| crate::Error::OAuth(format!("HTTP client error: {}", e)))?;

    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("code_verifier", code_verifier),
        ("client_id", config.client_id.as_str()),
    ];

    if !config.client_secret.is_empty() {
        form.push(("client_secret", config.client_secret.as_str()));
    }

    debug!("Exchanging authorization code for tokens");

    let resp = client
        .post(&config.token_url)
        .form(&form)
        .send()
        .await
        .map_err(|e| crate::Error::OAuth(format!("Token exchange request failed: {}", e)))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| crate::Error::OAuth(format!("Failed to read token response: {}", e)))?;

    if !status.is_success() {
        return Err(crate::Error::OAuth(format!(
            "Token exchange failed (HTTP {}): {}",
            status,
            crate::util::truncate_safe(&body, 200)
        )));
    }

    parse_token_response(&body, &config.token_file)
}

/// Refresh an expired access token using a refresh token.
pub async fn refresh_token(
    config: &OAuthProviderConfig,
    refresh_tok: &str,
) -> crate::Result<OAuthTokens> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| crate::Error::OAuth(format!("HTTP client error: {}", e)))?;

    let mut form = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_tok),
        ("client_id", config.client_id.as_str()),
    ];

    if !config.client_secret.is_empty() {
        form.push(("client_secret", config.client_secret.as_str()));
    }

    debug!("Refreshing OAuth token");

    let resp = client
        .post(&config.token_url)
        .form(&form)
        .send()
        .await
        .map_err(|e| crate::Error::OAuth(format!("Token refresh request failed: {}", e)))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| crate::Error::OAuth(format!("Failed to read refresh response: {}", e)))?;

    if !status.is_success() {
        return Err(crate::Error::OAuth(format!(
            "Token refresh failed (HTTP {}): {}",
            status,
            crate::util::truncate_safe(&body, 200)
        )));
    }

    let mut tokens = parse_token_response(&body, &config.token_file)?;
    // Preserve the original refresh token if the response didn't include a new one
    if tokens.refresh_token.is_none() {
        tokens.refresh_token = Some(refresh_tok.to_string());
    }
    Ok(tokens)
}

/// Save tokens to `~/.cratos/<filename>` with AES-256-GCM encryption and restrictive permissions.
pub fn save_tokens(filename: &str, tokens: &OAuthTokens) -> crate::Result<()> {
    let path = cratos_dir()?.join(filename);

    let json = serde_json::to_string_pretty(tokens)
        .map_err(|e| crate::Error::OAuth(format!("Failed to serialize tokens: {}", e)))?;

    let encrypted = encrypt_token_data(json.as_bytes())
        .map_err(|e| crate::Error::OAuth(format!("Failed to encrypt tokens: {}", e)))?;
    let encoded = URL_SAFE_NO_PAD.encode(&encrypted);

    std::fs::write(&path, &encoded)
        .map_err(|e| crate::Error::OAuth(format!("Failed to write {}: {}", path.display(), e)))?;

    // Set file permissions to 0600 (owner read/write only) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms).ok();
    }

    debug!("Saved encrypted OAuth tokens to {}", path.display());
    Ok(())
}

/// Read tokens from `~/.cratos/<filename>`.
///
/// Supports transparent migration: if the file contains plaintext JSON (legacy format),
/// it will be read successfully and automatically re-saved in encrypted format.
pub fn read_tokens(filename: &str) -> Option<OAuthTokens> {
    let path = cratos_dir().ok()?.join(filename);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            debug!("OAuth tokens not found at {}", path.display());
            return None;
        }
    };

    // Try encrypted format first: base64url decode → AES-256-GCM decrypt → JSON parse
    if let Ok(encrypted) = URL_SAFE_NO_PAD.decode(content.trim()) {
        if let Ok(decrypted) = decrypt_token_data(&encrypted) {
            if let Ok(tokens) = serde_json::from_slice::<OAuthTokens>(&decrypted) {
                if !tokens.access_token.is_empty() {
                    debug!("Read encrypted OAuth tokens from {}", path.display());
                    return Some(tokens);
                }
            }
        }
    }

    // Fallback: try plaintext JSON (legacy format)
    match serde_json::from_str::<OAuthTokens>(&content) {
        Ok(tokens) if !tokens.access_token.is_empty() => {
            debug!("Read legacy plaintext OAuth tokens from {}", path.display());
            // Auto-migrate to encrypted format
            if let Err(e) = save_tokens(filename, &tokens) {
                warn!("Failed to auto-migrate tokens to encrypted format: {}", e);
            } else {
                debug!("Auto-migrated tokens to encrypted format");
            }
            Some(tokens)
        }
        Ok(_) => {
            debug!("OAuth token file has empty access_token");
            None
        }
        Err(e) => {
            warn!(
                "Failed to parse OAuth tokens from {}: {}",
                path.display(),
                e
            );
            None
        }
    }
}

// ── Token Encryption (AES-256-GCM) ──

/// Derive a 256-bit encryption key for token storage.
///
/// Uses `CRATOS_MASTER_KEY` env var if set, otherwise falls back to
/// machine-specific data (hostname + username) for basic protection.
/// Uses a separate salt from credentials.rs to keep key spaces independent.
fn derive_token_encryption_key() -> [u8; 32] {
    let master_key = std::env::var("CRATOS_MASTER_KEY").unwrap_or_else(|_| {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "cratos-default".to_string());
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "cratos-user".to_string());
        format!("cratos-auto-key-{}-{}", hostname, username)
    });

    let mut hasher = Sha256::new();
    hasher.update(master_key.as_bytes());
    hasher.update(b"cratos-oauth-token-store-v1");
    let result = hasher.finalize();

    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypt data using AES-256-GCM with a random 12-byte nonce.
/// Returns nonce (12 bytes) || ciphertext.
fn encrypt_token_data(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let key_bytes = derive_token_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

/// Decrypt data encrypted with `encrypt_token_data`.
/// Expects nonce (12 bytes) || ciphertext.
fn decrypt_token_data(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    if encrypted.len() < 12 {
        return Err("Invalid encrypted data: too short".to_string());
    }

    let key_bytes = derive_token_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))
}

// ── Helpers ──

/// Get (and create if needed) the `~/.cratos/` directory.
fn cratos_dir() -> crate::Result<PathBuf> {
    let dir = dirs::home_dir()
        .ok_or_else(|| crate::Error::OAuth("Home directory not found".to_string()))?
        .join(".cratos");

    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            crate::Error::OAuth(format!("Failed to create {}: {}", dir.display(), e))
        })?;
    }
    Ok(dir)
}

/// Parse a token response JSON body into `OAuthTokens`.
fn parse_token_response(body: &str, provider: &str) -> crate::Result<OAuthTokens> {
    let json: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| crate::Error::OAuth(format!("Invalid token response JSON: {}", e)))?;

    let access_token = json["access_token"]
        .as_str()
        .ok_or_else(|| crate::Error::OAuth("Missing access_token in response".to_string()))?
        .to_string();

    let refresh_token = json["refresh_token"].as_str().map(String::from);

    // Calculate expiry: now + expires_in seconds
    let expiry_date = json["expires_in"]
        .as_i64()
        .map(|secs| chrono::Utc::now().timestamp_millis() + (secs * 1000))
        .unwrap_or(0);

    Ok(OAuthTokens {
        access_token,
        refresh_token,
        expiry_date,
        provider: provider.to_string(),
    })
}

/// Simple URL encoding (enough for OAuth params).
fn urlencoded(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            ' ' => result.push_str("%20"),
            _ => {
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                for b in encoded.bytes() {
                    result.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pkce() {
        let pkce = generate_pkce();
        // code_verifier should be base64url-encoded 64 bytes = 86 chars
        assert!(pkce.code_verifier.len() >= 43);
        // code_challenge should be base64url-encoded SHA-256 = 43 chars
        assert_eq!(pkce.code_challenge.len(), 43);
        // S256: challenge = base64url(sha256(verifier))
        let mut hasher = Sha256::new();
        hasher.update(pkce.code_verifier.as_bytes());
        let expected = URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert_eq!(pkce.code_challenge, expected);
    }

    #[test]
    fn test_generate_state() {
        let state1 = generate_state();
        let state2 = generate_state();
        // base64url-encoded 32 bytes = 43 chars
        assert_eq!(state1.len(), 43);
        // Each call produces a unique value
        assert_ne!(state1, state2);
    }

    #[test]
    fn test_build_auth_url() {
        let config = OAuthProviderConfig {
            client_id: "test-client".to_string(),
            client_secret: String::new(),
            auth_url: "https://example.com/auth".to_string(),
            token_url: "https://example.com/token".to_string(),
            scopes: "openid email".to_string(),
            redirect_path: "/callback".to_string(),
            extra_auth_params: vec![("access_type".to_string(), "offline".to_string())],
            token_file: "test.json".to_string(),
        };
        let pkce = PkcePair {
            code_verifier: "verifier".to_string(),
            code_challenge: "challenge".to_string(),
        };

        let url = build_auth_url(
            &config,
            "http://127.0.0.1:9999/callback",
            &pkce,
            "test-state",
        );
        assert!(url.starts_with("https://example.com/auth?"));
        assert!(url.contains("client_id=test-client"));
        assert!(url.contains("code_challenge=challenge"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=test-state"));
        assert!(url.contains("access_type=offline"));
    }

    #[test]
    fn test_parse_token_response() {
        let body = r#"{
            "access_token": "ya29.test",
            "refresh_token": "1//refresh",
            "expires_in": 3600,
            "token_type": "Bearer"
        }"#;

        let tokens = parse_token_response(body, "google_oauth.json").unwrap();
        assert_eq!(tokens.access_token, "ya29.test");
        assert_eq!(tokens.refresh_token.as_deref(), Some("1//refresh"));
        assert!(tokens.expiry_date > 0);
    }

    #[test]
    fn test_urlencoded() {
        assert_eq!(urlencoded("hello world"), "hello%20world");
        assert_eq!(urlencoded("foo@bar.com"), "foo%40bar.com");
        assert_eq!(urlencoded("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = b"secret token data for testing";
        let encrypted = encrypt_token_data(plaintext).unwrap();
        // Encrypted should be longer (nonce + ciphertext + tag)
        assert!(encrypted.len() > plaintext.len());
        let decrypted = decrypt_token_data(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_invalid_data() {
        // Too short
        assert!(decrypt_token_data(&[0u8; 5]).is_err());
        // Random data (wrong key / corrupted)
        let mut bad = [0u8; 64];
        getrandom::getrandom(&mut bad).unwrap();
        assert!(decrypt_token_data(&bad).is_err());
    }
}
