//! Credential backend types

use serde::{Deserialize, Serialize};

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
