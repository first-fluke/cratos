//! Tests for credentials module

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
