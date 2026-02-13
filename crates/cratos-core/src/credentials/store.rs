//! Credential store implementation

use super::backend::CredentialBackend;
use super::credential::Credential;
use super::encrypted_file::EncryptedFileBackend;
use super::error::{credential_key, handle_lock_poison, CredentialError, Result};
use super::keychain::KeychainBackend;
use super::secret_service::SecretServiceBackend;
use super::secure_string::SecureString;
use super::windows::WindowsBackend;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::{debug, info};

/// Secure credential storage
pub struct CredentialStore {
    backend: CredentialBackend,
    /// In-memory cache (for Memory backend and caching)
    pub(crate) cache: RwLock<HashMap<String, Credential>>,
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
    pub(crate) fn full_service(&self, service: &str) -> String {
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
            CredentialBackend::Keychain => {
                KeychainBackend::store(&full_service, account, value)
            }
            CredentialBackend::SecretService => {
                SecretServiceBackend::store(self, &full_service, account, value)
            }
            CredentialBackend::WindowsCredential => {
                WindowsBackend::store(self, &full_service, account, value)
            }
            CredentialBackend::EncryptedFile => {
                EncryptedFileBackend::store(self, &full_service, account, value)
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
            CredentialBackend::Keychain => KeychainBackend::get(&full_service, account),
            CredentialBackend::SecretService => {
                SecretServiceBackend::get(self, &full_service, account)
            }
            CredentialBackend::WindowsCredential => {
                WindowsBackend::get(self, &full_service, account)
            }
            CredentialBackend::EncryptedFile => {
                EncryptedFileBackend::get(self, &full_service, account)
            }
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
            CredentialBackend::Keychain => KeychainBackend::delete(&full_service, account),
            CredentialBackend::SecretService => {
                SecretServiceBackend::delete(self, &full_service, account)
            }
            CredentialBackend::WindowsCredential => {
                WindowsBackend::delete(self, &full_service, account)
            }
            CredentialBackend::EncryptedFile => {
                EncryptedFileBackend::delete(self, &full_service, account)
            }
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
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Get an API key from environment or credential store
pub fn get_api_key(store: &CredentialStore, provider: &str, env_var: &str) -> Result<SecureString> {
    store.get_or_env(provider, "api_key", env_var)
}
