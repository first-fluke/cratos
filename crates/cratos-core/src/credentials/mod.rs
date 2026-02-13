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

mod backend;
mod credential;
mod encrypted_file;
mod error;
mod keychain;
mod secret_service;
mod secure_string;
mod store;
mod windows;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use backend::CredentialBackend;
pub use credential::Credential;
pub use error::{CredentialError, Result};
pub use secure_string::SecureString;
pub use store::{get_api_key, CredentialStore};
