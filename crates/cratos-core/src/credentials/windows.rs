//! Windows Credential Manager backend

use super::encrypted_file::EncryptedFileBackend;
use super::error::Result;
use super::secure_string::SecureString;
use super::store::CredentialStore;
use tracing::warn;

pub struct WindowsBackend;

impl WindowsBackend {
    #[cfg(feature = "native-keyring")]
    pub fn store(store: &CredentialStore, service: &str, account: &str, value: &str) -> Result<()> {
        use super::error::CredentialError;

        match keyring::Entry::new(service, account) {
            Ok(entry) => entry.set_password(value).map_err(|e| {
                warn!(error = %e, "Windows Credential store failed, falling back to encrypted file");
                CredentialError::Backend(format!("Windows Credential error: {}", e))
            }),
            Err(e) => {
                warn!(error = %e, "Windows Credential unavailable, falling back to encrypted file");
                EncryptedFileBackend::store(store, service, account, value)
            }
        }
        .or_else(|_| EncryptedFileBackend::store(store, service, account, value))
    }

    #[cfg(feature = "native-keyring")]
    pub fn get(store: &CredentialStore, service: &str, account: &str) -> Result<SecureString> {
        match keyring::Entry::new(service, account) {
            Ok(entry) => match entry.get_password() {
                Ok(pw) => Ok(SecureString::new(pw)),
                Err(_) => EncryptedFileBackend::get(store, service, account),
            },
            Err(_) => EncryptedFileBackend::get(store, service, account),
        }
    }

    #[cfg(feature = "native-keyring")]
    pub fn delete(store: &CredentialStore, service: &str, account: &str) -> Result<()> {
        if let Ok(entry) = keyring::Entry::new(service, account) {
            let _ = entry.delete_credential();
        }
        let _ = EncryptedFileBackend::delete(store, service, account);
        Ok(())
    }

    #[cfg(not(feature = "native-keyring"))]
    pub fn store(store: &CredentialStore, service: &str, account: &str, value: &str) -> Result<()> {
        warn!("Windows Credential not available (native-keyring feature disabled), using encrypted file");
        EncryptedFileBackend::store(store, service, account, value)
    }

    #[cfg(not(feature = "native-keyring"))]
    pub fn get(store: &CredentialStore, service: &str, account: &str) -> Result<SecureString> {
        EncryptedFileBackend::get(store, service, account)
    }

    #[cfg(not(feature = "native-keyring"))]
    pub fn delete(store: &CredentialStore, service: &str, account: &str) -> Result<()> {
        EncryptedFileBackend::delete(store, service, account)
    }
}
