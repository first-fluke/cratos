//! Credential Store - Secure credential management
//!
//! This module provides secure credential storage using OS-native keychains:
//! - macOS Keychain
//! - Linux Secret Service (GNOME Keyring)
//! - Windows Credential Manager
//! - Encrypted file fallback (AES-256-GCM)
//!
//! ## Security Features
//!
//! - **SecureString**: Uses `zeroize` crate for cryptographic memory wiping
//! - **OS Keychains**: Leverages platform-native secure storage
//! - **Debug Safety**: Sensitive values are redacted in Debug output

#![forbid(unsafe_code)]

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use subtle::ConstantTimeEq;
use thiserror::Error;
use tracing::{debug, info, warn};
use zeroize::{Zeroize, ZeroizeOnDrop};

// ============================================================================
// Helper Functions (DRY)
// ============================================================================

/// Handle RwLock poison errors consistently
fn handle_lock_poison<T>(e: std::sync::PoisonError<T>) -> CredentialError {
    CredentialError::Backend(format!("Lock poisoned: {}", e))
}

/// Generate credential key from service and account
fn credential_key(service: &str, account: &str) -> String {
    format!("{}:{}", service, account)
}

// ============================================================================
// Error Types
// ============================================================================

/// Credential store errors
#[derive(Debug, Error)]
pub enum CredentialError {
    /// Credential not found
    #[error("Credential not found: {0}")]
    NotFound(String),

    /// Backend error
    #[error("Backend error: {0}")]
    Backend(String),

    /// Encryption error
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Access denied
    #[error("Access denied: {0}")]
    AccessDenied(String),
}

/// Result type for credential operations
pub type Result<T> = std::result::Result<T, CredentialError>;

// ============================================================================
// Credential Backend
// ============================================================================

/// Supported credential backends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialBackend {
    /// Automatic selection based on platform
    #[default]
    Auto,
    /// macOS Keychain
    Keychain,
    /// Linux Secret Service (D-Bus)
    SecretService,
    /// Windows Credential Manager
    WindowsCredential,
    /// Encrypted file fallback
    EncryptedFile,
    /// In-memory only (for testing)
    Memory,
}

impl CredentialBackend {
    /// Detect the best backend for the current platform
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::Keychain
        }
        #[cfg(target_os = "linux")]
        {
            Self::SecretService
        }
        #[cfg(target_os = "windows")]
        {
            Self::WindowsCredential
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Self::EncryptedFile
        }
    }

    /// Resolve Auto to actual backend
    #[must_use]
    pub fn resolve(self) -> Self {
        match self {
            Self::Auto => Self::detect(),
            other => other,
        }
    }
}

// ============================================================================
// Secure String
// ============================================================================

/// A string that is cryptographically cleared from memory when dropped
///
/// Uses the `zeroize` crate for secure memory wiping, which:
/// - Overwrites memory with zeros before deallocation
/// - Prevents compiler optimizations from removing the zeroing
/// - Works without unsafe code
///
/// # Security
///
/// - Value is zeroized on drop (via `ZeroizeOnDrop`)
/// - Debug and Display implementations redact the value
/// - Clone creates a new secure copy
///
/// # Example
///
/// ```
/// use cratos_core::credentials::SecureString;
///
/// let secret = SecureString::new("api-key-12345");
/// assert_eq!(secret.expose(), "api-key-12345");
///
/// // Debug output is redacted
/// let debug = format!("{:?}", secret);
/// assert!(!debug.contains("api-key"));
/// ```
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecureString {
    inner: String,
}

impl SecureString {
    /// Create a new secure string
    ///
    /// The value will be cryptographically zeroed when the SecureString is dropped.
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self { inner: s.into() }
    }

    /// Temporarily expose the string value
    ///
    /// # Security Warning
    ///
    /// The returned reference should not be stored or cloned unnecessarily.
    /// Prefer using this in limited scopes.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.inner
    }

    /// Get the length of the secret
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the secret is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Explicitly zeroize the string and replace with empty
    ///
    /// This can be called before drop if you want to clear the secret early.
    pub fn clear(&mut self) {
        self.inner.zeroize();
    }
}

impl std::fmt::Debug for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecureString([REDACTED, {} bytes])", self.inner.len())
    }
}

impl std::fmt::Display for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

// Prevent accidental comparison that might leak timing info
impl PartialEq for SecureString {
    fn eq(&self, other: &Self) -> bool {
        // Use constant-time comparison to prevent timing attacks
        self.inner.as_bytes().ct_eq(other.inner.as_bytes()).into()
    }
}

impl Eq for SecureString {}

// ============================================================================
// Credential Entry
// ============================================================================

/// Serializable credential for encrypted file storage
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedCredential {
    service: String,
    account: String,
    value: String,
}

/// A stored credential
#[derive(Debug, Clone)]
pub struct Credential {
    /// Service name (e.g., "cratos-openai")
    pub service: String,
    /// Account/username
    pub account: String,
    /// The secret value
    value: SecureString,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

impl Credential {
    /// Create a new credential
    #[must_use]
    pub fn new(
        service: impl Into<String>,
        account: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            service: service.into(),
            account: account.into(),
            value: SecureString::new(value),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get the secret value
    #[must_use]
    pub fn value(&self) -> &str {
        self.value.expose()
    }

    /// Generate the keyring key
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}:{}", self.service, self.account)
    }
}

// ============================================================================
// Credential Store
// ============================================================================

/// Secure credential storage
pub struct CredentialStore {
    backend: CredentialBackend,
    /// In-memory cache (for Memory backend and caching)
    cache: RwLock<HashMap<String, Credential>>,
    /// Service prefix for namespacing
    service_prefix: String,
}

impl CredentialStore {
    /// Create a new credential store with auto-detected backend
    #[must_use]
    pub fn new() -> Self {
        Self::with_backend(CredentialBackend::Auto)
    }

    /// Create with a specific backend
    #[must_use]
    pub fn with_backend(backend: CredentialBackend) -> Self {
        let resolved = backend.resolve();
        info!(backend = ?resolved, "Initializing credential store");

        Self {
            backend: resolved,
            cache: RwLock::new(HashMap::new()),
            service_prefix: "cratos".to_string(),
        }
    }

    /// Create an in-memory store (for testing)
    #[must_use]
    pub fn in_memory() -> Self {
        Self::with_backend(CredentialBackend::Memory)
    }

    /// Set the service prefix
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.service_prefix = prefix.into();
        self
    }

    /// Get the full service name
    fn full_service(&self, service: &str) -> String {
        format!("{}-{}", self.service_prefix, service)
    }

    /// Store a credential
    pub fn store(&self, service: &str, account: &str, value: &str) -> Result<()> {
        let full_service = self.full_service(service);
        let credential = Credential::new(&full_service, account, value);
        let key = credential.key();

        debug!(service = %full_service, account = %account, "Storing credential");

        match self.backend {
            CredentialBackend::Memory => {
                let mut cache = self.cache.write().map_err(handle_lock_poison)?;
                cache.insert(key, credential);
                Ok(())
            }
            CredentialBackend::Keychain => self.store_keychain(&full_service, account, value),
            CredentialBackend::SecretService => {
                self.store_secret_service(&full_service, account, value)
            }
            CredentialBackend::WindowsCredential => {
                self.store_windows(&full_service, account, value)
            }
            CredentialBackend::EncryptedFile => {
                self.store_encrypted_file(&full_service, account, value)
            }
            CredentialBackend::Auto => {
                // Should never happen after resolve()
                Err(CredentialError::Configuration(
                    "Backend not resolved".to_string(),
                ))
            }
        }
    }

    /// Retrieve a credential
    pub fn get(&self, service: &str, account: &str) -> Result<SecureString> {
        let full_service = self.full_service(service);
        let key = credential_key(&full_service, account);

        debug!(service = %full_service, account = %account, "Retrieving credential");

        match self.backend {
            CredentialBackend::Memory => {
                let cache = self.cache.read().map_err(handle_lock_poison)?;
                cache
                    .get(&key)
                    .map(|c| SecureString::new(c.value()))
                    .ok_or(CredentialError::NotFound(key))
            }
            CredentialBackend::Keychain => self.get_keychain(&full_service, account),
            CredentialBackend::SecretService => self.get_secret_service(&full_service, account),
            CredentialBackend::WindowsCredential => self.get_windows(&full_service, account),
            CredentialBackend::EncryptedFile => self.get_encrypted_file(&full_service, account),
            CredentialBackend::Auto => Err(CredentialError::Configuration(
                "Backend not resolved".to_string(),
            )),
        }
    }

    /// Delete a credential
    pub fn delete(&self, service: &str, account: &str) -> Result<()> {
        let full_service = self.full_service(service);
        let key = credential_key(&full_service, account);

        debug!(service = %full_service, account = %account, "Deleting credential");

        match self.backend {
            CredentialBackend::Memory => {
                let mut cache = self.cache.write().map_err(handle_lock_poison)?;
                cache.remove(&key);
                Ok(())
            }
            CredentialBackend::Keychain => self.delete_keychain(&full_service, account),
            CredentialBackend::SecretService => self.delete_secret_service(&full_service, account),
            CredentialBackend::WindowsCredential => self.delete_windows(&full_service, account),
            CredentialBackend::EncryptedFile => self.delete_encrypted_file(&full_service, account),
            CredentialBackend::Auto => Err(CredentialError::Configuration(
                "Backend not resolved".to_string(),
            )),
        }
    }

    /// Check if a credential exists
    pub fn exists(&self, service: &str, account: &str) -> bool {
        self.get(service, account).is_ok()
    }

    /// Get credential from environment variable, falling back to store
    pub fn get_or_env(&self, service: &str, account: &str, env_var: &str) -> Result<SecureString> {
        // First try environment variable
        if let Ok(value) = std::env::var(env_var) {
            debug!(env_var = %env_var, "Using credential from environment");
            return Ok(SecureString::new(value));
        }

        // Fall back to credential store
        self.get(service, account)
    }

    // ========================================================================
    // Platform-specific implementations
    // ========================================================================

    #[cfg(target_os = "macos")]
    fn store_keychain(&self, service: &str, account: &str, value: &str) -> Result<()> {
        use std::process::Command;

        let output = Command::new("security")
            .args([
                "add-generic-password",
                "-U", // Update if exists
                "-s",
                service,
                "-a",
                account,
                "-w",
                value,
            ])
            .output()
            .map_err(|e| CredentialError::Backend(format!("Failed to run security: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CredentialError::Backend(format!(
                "Keychain error: {}",
                stderr
            )));
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn store_keychain(&self, _service: &str, _account: &str, _value: &str) -> Result<()> {
        Err(CredentialError::Configuration(
            "Keychain only available on macOS".to_string(),
        ))
    }

    #[cfg(target_os = "macos")]
    fn get_keychain(&self, service: &str, account: &str) -> Result<SecureString> {
        use std::process::Command;

        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                service,
                "-a",
                account,
                "-w", // Print password only
            ])
            .output()
            .map_err(|e| CredentialError::Backend(format!("Failed to run security: {}", e)))?;

        if !output.status.success() {
            return Err(CredentialError::NotFound(credential_key(service, account)));
        }

        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(SecureString::new(value))
    }

    #[cfg(not(target_os = "macos"))]
    fn get_keychain(&self, _service: &str, _account: &str) -> Result<SecureString> {
        Err(CredentialError::Configuration(
            "Keychain only available on macOS".to_string(),
        ))
    }

    #[cfg(target_os = "macos")]
    fn delete_keychain(&self, service: &str, account: &str) -> Result<()> {
        use std::process::Command;

        let output = Command::new("security")
            .args(["delete-generic-password", "-s", service, "-a", account])
            .output()
            .map_err(|e| CredentialError::Backend(format!("Failed to run security: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "not found" errors
            if !stderr.contains("could not be found") {
                return Err(CredentialError::Backend(format!(
                    "Keychain error: {}",
                    stderr
                )));
            }
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn delete_keychain(&self, _service: &str, _account: &str) -> Result<()> {
        Err(CredentialError::Configuration(
            "Keychain only available on macOS".to_string(),
        ))
    }

    // Linux Secret Service stubs
    fn store_secret_service(&self, service: &str, account: &str, value: &str) -> Result<()> {
        // In a full implementation, this would use the Secret Service D-Bus API
        // For now, fall back to encrypted file
        warn!("Secret Service not implemented, falling back to encrypted file");
        self.store_encrypted_file(service, account, value)
    }

    fn get_secret_service(&self, service: &str, account: &str) -> Result<SecureString> {
        warn!("Secret Service not implemented, falling back to encrypted file");
        self.get_encrypted_file(service, account)
    }

    fn delete_secret_service(&self, service: &str, account: &str) -> Result<()> {
        warn!("Secret Service not implemented, falling back to encrypted file");
        self.delete_encrypted_file(service, account)
    }

    // Windows Credential stubs
    fn store_windows(&self, service: &str, account: &str, value: &str) -> Result<()> {
        warn!("Windows Credential not implemented, falling back to encrypted file");
        self.store_encrypted_file(service, account, value)
    }

    fn get_windows(&self, service: &str, account: &str) -> Result<SecureString> {
        warn!("Windows Credential not implemented, falling back to encrypted file");
        self.get_encrypted_file(service, account)
    }

    fn delete_windows(&self, service: &str, account: &str) -> Result<()> {
        warn!("Windows Credential not implemented, falling back to encrypted file");
        self.delete_encrypted_file(service, account)
    }

    // ========================================================================
    // Encrypted File Backend - AES-256-GCM
    // ========================================================================

    /// Get the path to the encrypted credentials file
    fn get_credentials_file_path() -> Result<PathBuf> {
        let data_dir = dirs::data_dir().or_else(dirs::home_dir).ok_or_else(|| {
            CredentialError::Configuration("Cannot determine data directory".to_string())
        })?;

        let cratos_dir = data_dir.join(".cratos");
        Ok(cratos_dir.join("credentials.enc"))
    }

    /// Derive a 256-bit encryption key from the master key
    /// Uses SHA-256 for key derivation (in production, consider Argon2 or PBKDF2)
    fn derive_encryption_key() -> Result<[u8; 32]> {
        // Get master key from environment or generate machine-specific key
        let master_key = std::env::var("CRATOS_MASTER_KEY").unwrap_or_else(|_| {
            // Use machine-specific data for key derivation when no master key is set
            // This provides basic protection but users should set CRATOS_MASTER_KEY for security
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
        hasher.update(b"cratos-credential-store-v1"); // Salt
        let result = hasher.finalize();

        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        Ok(key)
    }

    /// Encrypt data using AES-256-GCM
    fn encrypt_data(plaintext: &[u8]) -> Result<Vec<u8>> {
        let key_bytes = Self::derive_encryption_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| CredentialError::Encryption(format!("Failed to create cipher: {}", e)))?;

        // Generate random 12-byte nonce
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CredentialError::Encryption(format!("Encryption failed: {}", e)))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    /// Decrypt data using AES-256-GCM
    fn decrypt_data(encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() < 12 {
            return Err(CredentialError::Encryption(
                "Invalid encrypted data".to_string(),
            ));
        }

        let key_bytes = Self::derive_encryption_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| CredentialError::Encryption(format!("Failed to create cipher: {}", e)))?;

        let (nonce_bytes, ciphertext) = encrypted.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CredentialError::Encryption(format!("Decryption failed: {}", e)))
    }

    /// Load all credentials from encrypted file
    fn load_encrypted_credentials() -> Result<HashMap<String, EncryptedCredential>> {
        let path = Self::get_credentials_file_path()?;

        if !path.exists() {
            return Ok(HashMap::new());
        }

        let encrypted_b64 = std::fs::read_to_string(&path).map_err(|e| {
            CredentialError::Backend(format!("Failed to read credentials file: {}", e))
        })?;

        let encrypted = BASE64.decode(encrypted_b64.trim()).map_err(|e| {
            CredentialError::Encryption(format!("Failed to decode credentials: {}", e))
        })?;

        let decrypted = Self::decrypt_data(&encrypted)?;

        let credentials: HashMap<String, EncryptedCredential> = serde_json::from_slice(&decrypted)
            .map_err(|e| CredentialError::Backend(format!("Failed to parse credentials: {}", e)))?;

        debug!(
            count = credentials.len(),
            "Loaded credentials from encrypted file"
        );
        Ok(credentials)
    }

    /// Save all credentials to encrypted file
    fn save_encrypted_credentials(
        credentials: &HashMap<String, EncryptedCredential>,
    ) -> Result<()> {
        let path = Self::get_credentials_file_path()?;

        // Ensure parent directory exists with secure permissions
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CredentialError::Backend(format!("Failed to create directory: {}", e))
            })?;

            // Set directory permissions to owner-only on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o700);
                let _ = std::fs::set_permissions(parent, perms);
            }
        }

        let json = serde_json::to_vec(credentials).map_err(|e| {
            CredentialError::Backend(format!("Failed to serialize credentials: {}", e))
        })?;

        let encrypted = Self::encrypt_data(&json)?;
        let encoded = BASE64.encode(&encrypted);

        std::fs::write(&path, encoded).map_err(|e| {
            CredentialError::Backend(format!("Failed to write credentials file: {}", e))
        })?;

        // Set file permissions to owner-only on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }

        debug!(path = %path.display(), "Saved credentials to encrypted file");
        Ok(())
    }

    /// Encrypted file backend - stores credentials with AES-256-GCM encryption
    fn store_encrypted_file(&self, service: &str, account: &str, value: &str) -> Result<()> {
        let key = credential_key(service, account);

        // Also cache in memory
        let credential = Credential::new(service, account, value);
        {
            let mut cache = self.cache.write().map_err(handle_lock_poison)?;
            cache.insert(key.clone(), credential);
        }

        // Load, update, and save encrypted file
        let mut credentials = Self::load_encrypted_credentials().unwrap_or_default();
        credentials.insert(
            key,
            EncryptedCredential {
                service: service.to_string(),
                account: account.to_string(),
                value: value.to_string(),
            },
        );
        Self::save_encrypted_credentials(&credentials)?;

        info!(service = %service, account = %account, "Credential stored with AES-256-GCM encryption");
        Ok(())
    }

    fn get_encrypted_file(&self, service: &str, account: &str) -> Result<SecureString> {
        let key = credential_key(service, account);

        // Try cache first
        {
            let cache = self.cache.read().map_err(handle_lock_poison)?;
            if let Some(cred) = cache.get(&key) {
                return Ok(SecureString::new(cred.value()));
            }
        }

        // Load from encrypted file
        let credentials = Self::load_encrypted_credentials()?;
        if let Some(cred) = credentials.get(&key) {
            // Update cache
            let credential = Credential::new(&cred.service, &cred.account, &cred.value);
            let mut cache = self.cache.write().map_err(handle_lock_poison)?;
            cache.insert(key, credential);

            Ok(SecureString::new(&cred.value))
        } else {
            Err(CredentialError::NotFound(credential_key(service, account)))
        }
    }

    fn delete_encrypted_file(&self, service: &str, account: &str) -> Result<()> {
        let key = credential_key(service, account);

        // Remove from cache
        {
            let mut cache = self.cache.write().map_err(handle_lock_poison)?;
            cache.remove(&key);
        }

        // Remove from encrypted file
        let mut credentials = Self::load_encrypted_credentials().unwrap_or_default();
        credentials.remove(&key);
        Self::save_encrypted_credentials(&credentials)?;

        info!(service = %service, account = %account, "Credential deleted from encrypted storage");
        Ok(())
    }
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Get an API key from environment or credential store
pub fn get_api_key(store: &CredentialStore, provider: &str, env_var: &str) -> Result<SecureString> {
    store.get_or_env(provider, "api_key", env_var)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_string() {
        let secret = SecureString::new("my-secret-value");
        assert_eq!(secret.expose(), "my-secret-value");
        assert_eq!(secret.len(), 15);
        assert!(!secret.is_empty());

        // Debug should not expose value
        let debug = format!("{:?}", secret);
        assert!(!debug.contains("my-secret-value"));
        assert!(debug.contains("REDACTED"));

        // Display should also be redacted
        let display = format!("{}", secret);
        assert!(!display.contains("my-secret-value"));
        assert!(display.contains("REDACTED"));
    }

    #[test]
    fn test_secure_string_equality() {
        let secret1 = SecureString::new("test-value");
        let secret2 = SecureString::new("test-value");
        let secret3 = SecureString::new("different-value");

        // Constant-time equality
        assert_eq!(secret1, secret2);
        assert_ne!(secret1, secret3);
    }

    #[test]
    fn test_secure_string_clear() {
        let mut secret = SecureString::new("sensitive-data");
        assert!(!secret.is_empty());

        secret.clear();
        assert!(secret.is_empty());
    }

    #[test]
    fn test_secure_string_clone() {
        let original = SecureString::new("clone-test");
        let cloned = original.clone();

        assert_eq!(original.expose(), cloned.expose());
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_credential() {
        let cred = Credential::new("openai", "api_key", "sk-test123")
            .with_metadata("created_at", "2024-01-01");

        assert_eq!(cred.service, "openai");
        assert_eq!(cred.account, "api_key");
        assert_eq!(cred.value(), "sk-test123");
        assert_eq!(cred.key(), "openai:api_key");
        assert_eq!(
            cred.metadata.get("created_at"),
            Some(&"2024-01-01".to_string())
        );
    }

    #[test]
    fn test_credential_backend_detect() {
        let backend = CredentialBackend::detect();
        // Should detect a valid backend for the current platform
        assert_ne!(backend, CredentialBackend::Auto);
    }

    #[test]
    fn test_memory_store() {
        let store = CredentialStore::in_memory();

        // Store and retrieve
        store.store("test", "user", "secret123").unwrap();
        let retrieved = store.get("test", "user").unwrap();
        assert_eq!(retrieved.expose(), "secret123");

        // Check exists
        assert!(store.exists("test", "user"));
        assert!(!store.exists("test", "other"));

        // Delete
        store.delete("test", "user").unwrap();
        assert!(!store.exists("test", "user"));
    }

    #[test]
    fn test_get_or_env() {
        let store = CredentialStore::in_memory();

        // Set env var
        std::env::set_var("TEST_CRED_VAR", "env-value");

        // Should get from env
        let result = store.get_or_env("test", "user", "TEST_CRED_VAR").unwrap();
        assert_eq!(result.expose(), "env-value");

        // Clean up
        std::env::remove_var("TEST_CRED_VAR");

        // Now should fail (no env, no stored)
        assert!(store.get_or_env("test", "user", "TEST_CRED_VAR").is_err());

        // Store a value
        store.store("test", "user", "stored-value").unwrap();

        // Should get from store
        let result = store.get_or_env("test", "user", "TEST_CRED_VAR").unwrap();
        assert_eq!(result.expose(), "stored-value");
    }

    #[test]
    fn test_service_prefix() {
        let store = CredentialStore::in_memory().with_prefix("myapp");
        store.store("openai", "key", "secret").unwrap();

        // The internal key should include the prefix
        let cache = store.cache.read().unwrap();
        assert!(cache.contains_key("myapp-openai:key"));
    }
}
