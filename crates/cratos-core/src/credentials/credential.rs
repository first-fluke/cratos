//! Credential types

use super::secure_string::SecureString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Serializable credential for encrypted file storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCredential {
    pub service: String,
    pub account: String,
    pub value: String,
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
