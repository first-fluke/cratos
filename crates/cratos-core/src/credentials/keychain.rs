//! macOS Keychain backend

use super::error::{credential_key, CredentialError, Result};
use super::secure_string::SecureString;

pub struct KeychainBackend;

impl KeychainBackend {
    #[cfg(target_os = "macos")]
    pub fn store(service: &str, account: &str, value: &str) -> Result<()> {
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
    pub fn store(_service: &str, _account: &str, _value: &str) -> Result<()> {
        Err(CredentialError::Configuration(
            "Keychain only available on macOS".to_string(),
        ))
    }

    #[cfg(target_os = "macos")]
    pub fn get(service: &str, account: &str) -> Result<SecureString> {
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
    pub fn get(_service: &str, _account: &str) -> Result<SecureString> {
        Err(CredentialError::Configuration(
            "Keychain only available on macOS".to_string(),
        ))
    }

    #[cfg(target_os = "macos")]
    pub fn delete(service: &str, account: &str) -> Result<()> {
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
    pub fn delete(_service: &str, _account: &str) -> Result<()> {
        Err(CredentialError::Configuration(
            "Keychain only available on macOS".to_string(),
        ))
    }
}
