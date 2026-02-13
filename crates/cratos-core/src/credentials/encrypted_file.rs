//! Encrypted file backend using AES-256-GCM

use super::credential::{Credential, EncryptedCredential};
use super::error::{credential_key, handle_lock_poison, CredentialError, Result};
use super::secure_string::SecureString;
use super::store::CredentialStore;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info};

pub struct EncryptedFileBackend;

impl EncryptedFileBackend {
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

    /// Store credential with AES-256-GCM encryption
    pub fn store(
        credential_store: &CredentialStore,
        service: &str,
        account: &str,
        value: &str,
    ) -> Result<()> {
        let key = credential_key(service, account);

        // Also cache in memory
        let credential = Credential::new(service, account, value);
        {
            let mut cache = credential_store.cache.write().map_err(handle_lock_poison)?;
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

    pub fn get(
        credential_store: &CredentialStore,
        service: &str,
        account: &str,
    ) -> Result<SecureString> {
        let key = credential_key(service, account);

        // Try cache first
        {
            let cache = credential_store.cache.read().map_err(handle_lock_poison)?;
            if let Some(cred) = cache.get(&key) {
                return Ok(SecureString::new(cred.value()));
            }
        }

        // Load from encrypted file
        let credentials = Self::load_encrypted_credentials()?;
        if let Some(cred) = credentials.get(&key) {
            // Update cache
            let credential = Credential::new(&cred.service, &cred.account, &cred.value);
            let mut cache = credential_store.cache.write().map_err(handle_lock_poison)?;
            cache.insert(key, credential);

            Ok(SecureString::new(&cred.value))
        } else {
            Err(CredentialError::NotFound(credential_key(service, account)))
        }
    }

    pub fn delete(
        credential_store: &CredentialStore,
        service: &str,
        account: &str,
    ) -> Result<()> {
        let key = credential_key(service, account);

        // Remove from cache
        {
            let mut cache = credential_store.cache.write().map_err(handle_lock_poison)?;
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
