//! External authentication providers.
//!
//! Provides a trait-based interface for plugging in external identity providers
//! (Tailscale, Google, GitHub, etc.) following multi-auth pattern.

#![forbid(unsafe_code)]

use crate::auth::Scope;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::debug;

/// Result of a successful external authentication.
#[derive(Debug, Clone)]
pub struct ExternalAuthResult {
    /// User ID from the external provider
    pub user_id: String,
    /// Display name (if available)
    pub display_name: Option<String>,
    /// Scopes granted by this provider
    pub scopes: Vec<Scope>,
    /// Provider-specific metadata
    pub metadata: HashMap<String, String>,
}

/// Error from external authentication.
#[derive(Debug, Clone)]
pub enum ExternalAuthError {
    /// Provider is not available or not configured
    Unavailable(String),
    /// Credential verification failed
    VerificationFailed(String),
    /// Network or IO error
    NetworkError(String),
}

impl std::fmt::Display for ExternalAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(msg) => write!(f, "provider unavailable: {}", msg),
            Self::VerificationFailed(msg) => write!(f, "verification failed: {}", msg),
            Self::NetworkError(msg) => write!(f, "network error: {}", msg),
        }
    }
}

impl std::error::Error for ExternalAuthError {}

/// Trait for external authentication providers.
///
/// Implementations verify credentials against an external identity service
/// and return user information if successful.
#[async_trait]
pub trait ExternalAuthVerifier: Send + Sync {
    /// Provider name (e.g., "tailscale", "google", "github").
    fn provider_name(&self) -> &str;

    /// Verify a credential string and return user info.
    ///
    /// The credential format is provider-specific:
    /// - Tailscale: peer IP address
    /// - OAuth: access token
    async fn verify(&self, credential: &str) -> Result<ExternalAuthResult, ExternalAuthError>;
}

/// Registry of external authentication verifiers.
pub struct ExternalAuthRegistry {
    verifiers: HashMap<String, Box<dyn ExternalAuthVerifier>>,
}

impl ExternalAuthRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            verifiers: HashMap::new(),
        }
    }

    /// Register a verifier.
    pub fn register(&mut self, verifier: Box<dyn ExternalAuthVerifier>) {
        let name = verifier.provider_name().to_string();
        debug!(provider = %name, "Registered external auth provider");
        self.verifiers.insert(name, verifier);
    }

    /// Check if a provider is registered.
    pub fn has_provider(&self, provider: &str) -> bool {
        self.verifiers.contains_key(provider)
    }

    /// List registered provider names.
    pub fn providers(&self) -> Vec<&str> {
        self.verifiers.keys().map(|s| s.as_str()).collect()
    }

    /// Verify credentials against a specific provider.
    pub async fn verify(
        &self,
        provider: &str,
        credential: &str,
    ) -> Result<ExternalAuthResult, ExternalAuthError> {
        let verifier = self.verifiers.get(provider).ok_or_else(|| {
            ExternalAuthError::Unavailable(format!("provider '{}' not registered", provider))
        })?;

        verifier.verify(credential).await
    }
}

impl Default for ExternalAuthRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Tailscale LocalAPI verifier.
///
/// Verifies peer identity by querying the Tailscale daemon's local API
/// (`/localapi/v0/whois?addr=<peer_ip>`). Only works when:
/// - The Cratos server is running on a Tailscale node
/// - The tailscaled socket is accessible
///
/// This is the Tailscale SSO pattern from 4-layer auth.
pub struct TailscaleVerifier {
    socket_path: String,
}

impl TailscaleVerifier {
    /// Create with default socket path.
    pub fn new() -> Self {
        Self {
            socket_path: "/var/run/tailscale/tailscaled.sock".to_string(),
        }
    }

    /// Create with custom socket path.
    pub fn with_socket(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }
}

impl Default for TailscaleVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Tailscale whois response (simplified)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TailscaleWhoisResponse {
    node: Option<TailscaleNode>,
    user_profile: Option<TailscaleUserProfile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TailscaleNode {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TailscaleUserProfile {
    login_name: String,
    display_name: String,
}

#[async_trait]
impl ExternalAuthVerifier for TailscaleVerifier {
    fn provider_name(&self) -> &str {
        "tailscale"
    }

    async fn verify(&self, peer_ip: &str) -> Result<ExternalAuthResult, ExternalAuthError> {
        if !std::path::Path::new(&self.socket_path).exists() {
            return Err(ExternalAuthError::Unavailable(
                "tailscaled socket not found".to_string(),
            ));
        }

        let whois = query_tailscale_whois(&self.socket_path, peer_ip).await?;

        let user_id = whois
            .user_profile
            .as_ref()
            .map(|p| p.login_name.clone())
            .unwrap_or_else(|| peer_ip.to_string());

        let display_name = whois.user_profile.as_ref().map(|p| p.display_name.clone());

        let mut metadata = HashMap::new();
        if let Some(node) = &whois.node {
            metadata.insert("tailscale_node".to_string(), node.name.clone());
        }

        Ok(ExternalAuthResult {
            user_id,
            display_name,
            scopes: vec![
                Scope::SessionRead,
                Scope::SessionWrite,
                Scope::ExecutionRead,
                Scope::ExecutionWrite,
                Scope::ConfigRead,
            ],
            metadata,
        })
    }
}

/// Query tailscaled whois via Unix domain socket using raw HTTP/1.1
#[cfg(unix)]
async fn query_tailscale_whois(
    socket_path: &str,
    peer_ip: &str,
) -> Result<TailscaleWhoisResponse, ExternalAuthError> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path).await.map_err(|e| {
        ExternalAuthError::NetworkError(format!("Failed to connect to tailscaled: {}", e))
    })?;

    let request = format!(
        "GET /localapi/v0/whois?addr={} HTTP/1.1\r\nHost: local-tailscaled.sock\r\nConnection: close\r\n\r\n",
        peer_ip
    );

    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| ExternalAuthError::NetworkError(format!("Write failed: {}", e)))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .map_err(|e| ExternalAuthError::NetworkError(format!("Read failed: {}", e)))?;

    let response_str = String::from_utf8_lossy(&response);

    // Parse HTTP response: find body after \r\n\r\n
    let body = response_str
        .find("\r\n\r\n")
        .map(|pos| &response_str[pos + 4..])
        .unwrap_or(&response_str);

    serde_json::from_str(body).map_err(|e| {
        ExternalAuthError::VerificationFailed(format!("Invalid whois response: {}", e))
    })
}

#[cfg(not(unix))]
async fn query_tailscale_whois(
    _socket_path: &str,
    _peer_ip: &str,
) -> Result<TailscaleWhoisResponse, ExternalAuthError> {
    Err(ExternalAuthError::Unavailable(
        "Tailscale Unix socket only available on Unix".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock verifier for testing.
    struct MockVerifier {
        name: String,
        result: Result<ExternalAuthResult, ExternalAuthError>,
    }

    #[async_trait]
    impl ExternalAuthVerifier for MockVerifier {
        fn provider_name(&self) -> &str {
            &self.name
        }

        async fn verify(&self, _credential: &str) -> Result<ExternalAuthResult, ExternalAuthError> {
            self.result.clone()
        }
    }

    fn mock_success(name: &str) -> MockVerifier {
        MockVerifier {
            name: name.to_string(),
            result: Ok(ExternalAuthResult {
                user_id: "test-user".to_string(),
                display_name: Some("Test User".to_string()),
                scopes: vec![Scope::SessionRead, Scope::ExecutionRead],
                metadata: HashMap::new(),
            }),
        }
    }

    fn mock_failure(name: &str) -> MockVerifier {
        MockVerifier {
            name: name.to_string(),
            result: Err(ExternalAuthError::VerificationFailed(
                "invalid token".to_string(),
            )),
        }
    }

    #[tokio::test]
    async fn test_registry_register_and_verify() {
        let mut registry = ExternalAuthRegistry::new();
        registry.register(Box::new(mock_success("test-provider")));

        assert!(registry.has_provider("test-provider"));
        assert!(!registry.has_provider("other"));

        let result = registry.verify("test-provider", "any-credential").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().user_id, "test-user");
    }

    #[tokio::test]
    async fn test_registry_unknown_provider() {
        let registry = ExternalAuthRegistry::new();
        let result = registry.verify("unknown", "cred").await;
        assert!(matches!(result, Err(ExternalAuthError::Unavailable(_))));
    }

    #[tokio::test]
    async fn test_registry_verification_failure() {
        let mut registry = ExternalAuthRegistry::new();
        registry.register(Box::new(mock_failure("failing")));

        let result = registry.verify("failing", "cred").await;
        assert!(matches!(
            result,
            Err(ExternalAuthError::VerificationFailed(_))
        ));
    }

    #[tokio::test]
    async fn test_registry_providers_list() {
        let mut registry = ExternalAuthRegistry::new();
        registry.register(Box::new(mock_success("a")));
        registry.register(Box::new(mock_success("b")));

        let mut providers = registry.providers();
        providers.sort();
        assert_eq!(providers, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn test_tailscale_verifier_no_socket() {
        let verifier = TailscaleVerifier::with_socket("/nonexistent/path");
        let result = verifier.verify("100.64.1.1").await;
        assert!(matches!(result, Err(ExternalAuthError::Unavailable(_))));
    }
}
