//! Authentication and Authorization module
//!
//! Provides:
//! - API key and Bearer token authentication
//! - Scope-based authorization (OpenClaw pattern)
//! - Token generation, validation, and revocation
//! - Constant-time token comparison

#![forbid(unsafe_code)]

use crate::credentials::SecureString;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use subtle::ConstantTimeEq;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ============================================================================
// Error Types
// ============================================================================

/// Authentication/Authorization errors
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// No credentials provided
    #[error("Authentication required")]
    MissingCredentials,

    /// Invalid token or API key
    #[error("Invalid credentials")]
    InvalidCredentials,

    /// Token has been revoked
    #[error("Token revoked")]
    TokenRevoked,

    /// Insufficient permissions
    #[error("Insufficient scope: requires {required}")]
    InsufficientScope {
        /// The scope that was required
        required: String,
    },

    /// Internal error
    #[error("Auth internal error: {0}")]
    Internal(String),
}

/// Auth result type
pub type Result<T> = std::result::Result<T, AuthError>;

// ============================================================================
// Scopes
// ============================================================================

/// Scope-based authorization (OpenClaw pattern)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Full access to everything
    Admin,
    /// Read session data
    SessionRead,
    /// Create/modify/delete sessions
    SessionWrite,
    /// Read execution history
    ExecutionRead,
    /// Request new executions
    ExecutionWrite,
    /// Respond to approval requests
    ApprovalRespond,
    /// Read configuration
    ConfigRead,
    /// Modify configuration
    ConfigWrite,
    /// Register and manage nodes (Phase 5)
    NodeManage,
    /// Read scheduler tasks
    SchedulerRead,
    /// Create, update, delete scheduler tasks
    SchedulerWrite,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scope::Admin => write!(f, "admin"),
            Scope::SessionRead => write!(f, "session_read"),
            Scope::SessionWrite => write!(f, "session_write"),
            Scope::ExecutionRead => write!(f, "execution_read"),
            Scope::ExecutionWrite => write!(f, "execution_write"),
            Scope::ApprovalRespond => write!(f, "approval_respond"),
            Scope::ConfigRead => write!(f, "config_read"),
            Scope::ConfigWrite => write!(f, "config_write"),
            Scope::NodeManage => write!(f, "node_manage"),
            Scope::SchedulerRead => write!(f, "scheduler_read"),
            Scope::SchedulerWrite => write!(f, "scheduler_write"),
        }
    }
}

// ============================================================================
// Auth Method
// ============================================================================

/// Authentication method used
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Bearer token (REST API, WS handshake)
    BearerToken,
    /// API key (X-API-Key header)
    ApiKey,
    /// Ed25519 device signature (Phase 8)
    DeviceSignature {
        /// The authenticated device ID
        device_id: String,
    },
    /// External auth provider (Tailscale, OAuth, etc.)
    ExternalAuth {
        /// Provider name (e.g., "tailscale", "google")
        provider: String,
    },
}

// ============================================================================
// Auth Context
// ============================================================================

/// Authenticated context attached to each request
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// User identifier
    pub user_id: String,
    /// How the user authenticated
    pub method: AuthMethod,
    /// Granted scopes
    pub scopes: Vec<Scope>,
    /// Associated session ID (if any)
    pub session_id: Option<Uuid>,
    /// Device ID (set when authenticated via DeviceSignature)
    pub device_id: Option<String>,
}

impl AuthContext {
    /// Check if this context has a specific scope
    pub fn has_scope(&self, scope: &Scope) -> bool {
        self.scopes.contains(&Scope::Admin) || self.scopes.contains(scope)
    }

    /// Require a specific scope, returning error if missing
    pub fn require_scope(&self, scope: &Scope) -> Result<()> {
        if self.has_scope(scope) {
            Ok(())
        } else {
            Err(AuthError::InsufficientScope {
                required: scope.to_string(),
            })
        }
    }
}

// ============================================================================
// Stored API Key
// ============================================================================

/// Internal representation of a stored API key
#[derive(Debug, Clone)]
struct StoredKey {
    /// SHA-256 hash of the key (we never store the raw key)
    key_hash: [u8; 32],
    /// User ID this key belongs to
    user_id: String,
    /// Granted scopes
    scopes: Vec<Scope>,
    /// Human-readable label
    label: String,
    /// When the key was created
    created_at: DateTime<Utc>,
    /// Whether the key has been revoked
    revoked: bool,
}

// ============================================================================
// Auth Store
// ============================================================================

/// Token storage and validation
pub struct AuthStore {
    /// key_hash_hex → StoredKey
    keys: RwLock<HashMap<String, StoredKey>>,
    /// Whether auth is enabled
    enabled: bool,
}

impl AuthStore {
    /// Create a new auth store
    pub fn new(enabled: bool) -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
            enabled,
        }
    }

    /// Check if authentication is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Hash a token/key using SHA-256
    fn hash_key(key: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Convert hash to hex string for map lookup
    fn hash_to_hex(hash: &[u8; 32]) -> String {
        hash.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Generate a new API key for a user
    ///
    /// Returns the raw key (only shown once) and the key hash for reference.
    pub fn generate_api_key(
        &self,
        user_id: &str,
        scopes: Vec<Scope>,
        label: &str,
    ) -> Result<(SecureString, String)> {
        // Generate a random API key: cratos_<uuid>
        let raw_key = format!("cratos_{}", Uuid::new_v4().as_simple());
        let key_hash = Self::hash_key(&raw_key);
        let key_hash_hex = Self::hash_to_hex(&key_hash);

        let stored = StoredKey {
            key_hash,
            user_id: user_id.to_string(),
            scopes,
            label: label.to_string(),
            created_at: Utc::now(),
            revoked: false,
        };

        let mut keys = self
            .keys
            .write()
            .map_err(|e| AuthError::Internal(format!("Lock poisoned: {}", e)))?;
        keys.insert(key_hash_hex.clone(), stored);

        info!(
            user_id = %user_id,
            label = %label,
            key_prefix = %&raw_key[..12],
            "API key generated"
        );

        Ok((SecureString::new(raw_key), key_hash_hex))
    }

    /// Validate a token/API key and return the auth context
    pub fn validate_token(&self, token: &str) -> Result<AuthContext> {
        if !self.enabled {
            // When auth is disabled, return a default admin context
            return Ok(AuthContext {
                user_id: "anonymous".to_string(),
                method: AuthMethod::BearerToken,
                scopes: vec![Scope::Admin],
                session_id: None,
                device_id: None,
            });
        }

        if token.is_empty() {
            return Err(AuthError::MissingCredentials);
        }

        let token_hash = Self::hash_key(token);
        let token_hash_hex = Self::hash_to_hex(&token_hash);

        let keys = self
            .keys
            .read()
            .map_err(|e| AuthError::Internal(format!("Lock poisoned: {}", e)))?;

        // Find the stored key by hash
        if let Some(stored) = keys.get(&token_hash_hex) {
            // Constant-time comparison of the hash
            let hashes_match: bool = stored.key_hash.ct_eq(&token_hash).into();
            if !hashes_match {
                return Err(AuthError::InvalidCredentials);
            }

            if stored.revoked {
                return Err(AuthError::TokenRevoked);
            }

            debug!(user_id = %stored.user_id, label = %stored.label, "Token validated");

            let method = if token.starts_with("cratos_") {
                AuthMethod::ApiKey
            } else {
                AuthMethod::BearerToken
            };

            Ok(AuthContext {
                user_id: stored.user_id.clone(),
                method,
                scopes: stored.scopes.clone(),
                session_id: None,
                device_id: None,
            })
        } else {
            warn!("Invalid token attempt");
            Err(AuthError::InvalidCredentials)
        }
    }

    /// Revoke a key by its hash
    pub fn revoke_key(&self, key_hash_hex: &str) -> Result<()> {
        let mut keys = self
            .keys
            .write()
            .map_err(|e| AuthError::Internal(format!("Lock poisoned: {}", e)))?;

        if let Some(stored) = keys.get_mut(key_hash_hex) {
            stored.revoked = true;
            info!(
                user_id = %stored.user_id,
                label = %stored.label,
                "API key revoked"
            );
            Ok(())
        } else {
            Err(AuthError::InvalidCredentials)
        }
    }

    /// List all keys (non-sensitive info only)
    pub fn list_keys(&self) -> Result<Vec<ApiKeyInfo>> {
        let keys = self
            .keys
            .read()
            .map_err(|e| AuthError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(keys
            .iter()
            .map(|(hash_hex, stored)| ApiKeyInfo {
                key_hash: hash_hex.clone(),
                user_id: stored.user_id.clone(),
                label: stored.label.clone(),
                scopes: stored.scopes.clone(),
                created_at: stored.created_at,
                revoked: stored.revoked,
            })
            .collect())
    }

    /// Get count of active (non-revoked) keys
    pub fn active_key_count(&self) -> usize {
        self.keys
            .read()
            .map(|keys| keys.values().filter(|k| !k.revoked).count())
            .unwrap_or(0)
    }
}

/// Non-sensitive API key information for listing
#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyInfo {
    /// Hash of the key (for identification/revocation)
    pub key_hash: String,
    /// Owner user ID
    pub user_id: String,
    /// Human-readable label
    pub label: String,
    /// Granted scopes
    pub scopes: Vec<Scope>,
    /// Creation time
    pub created_at: DateTime<Utc>,
    /// Whether revoked
    pub revoked: bool,
}

// ============================================================================
// Default scopes helper
// ============================================================================

/// Default scopes for a new user API key (everything except admin and node)
pub fn default_user_scopes() -> Vec<Scope> {
    vec![
        Scope::SessionRead,
        Scope::SessionWrite,
        Scope::ExecutionRead,
        Scope::ExecutionWrite,
        Scope::ApprovalRespond,
        Scope::ConfigRead,
        Scope::SchedulerRead,
    ]
}

/// All scopes (admin)
pub fn admin_scopes() -> Vec<Scope> {
    vec![Scope::Admin]
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_validate_key() {
        let store = AuthStore::new(true);
        let (key, _hash) = store
            .generate_api_key("user1", default_user_scopes(), "test key")
            .unwrap();

        let ctx = store.validate_token(key.expose()).unwrap();
        assert_eq!(ctx.user_id, "user1");
        assert!(ctx.has_scope(&Scope::SessionRead));
        assert!(!ctx.has_scope(&Scope::Admin));
    }

    #[test]
    fn test_invalid_token() {
        let store = AuthStore::new(true);
        let result = store.validate_token("invalid_token");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_token() {
        let store = AuthStore::new(true);
        let result = store.validate_token("");
        assert!(matches!(result, Err(AuthError::MissingCredentials)));
    }

    #[test]
    fn test_revoke_key() {
        let store = AuthStore::new(true);
        let (key, hash) = store
            .generate_api_key("user1", default_user_scopes(), "test")
            .unwrap();

        // Should work before revocation
        assert!(store.validate_token(key.expose()).is_ok());

        // Revoke
        store.revoke_key(&hash).unwrap();

        // Should fail after revocation
        let result = store.validate_token(key.expose());
        assert!(matches!(result, Err(AuthError::TokenRevoked)));
    }

    #[test]
    fn test_disabled_auth() {
        let store = AuthStore::new(false);
        let ctx = store.validate_token("anything").unwrap();
        assert_eq!(ctx.user_id, "anonymous");
        assert!(ctx.has_scope(&Scope::Admin));
    }

    #[test]
    fn test_scope_check() {
        let ctx = AuthContext {
            user_id: "user1".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::SessionRead, Scope::ExecutionRead],
            session_id: None,
            device_id: None,
        };

        assert!(ctx.has_scope(&Scope::SessionRead));
        assert!(ctx.has_scope(&Scope::ExecutionRead));
        assert!(!ctx.has_scope(&Scope::Admin));
        assert!(!ctx.has_scope(&Scope::ConfigWrite));
    }

    #[test]
    fn test_admin_scope_grants_all() {
        let ctx = AuthContext {
            user_id: "admin".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::Admin],
            session_id: None,
            device_id: None,
        };

        assert!(ctx.has_scope(&Scope::SessionRead));
        assert!(ctx.has_scope(&Scope::ConfigWrite));
        assert!(ctx.has_scope(&Scope::NodeManage));
    }

    #[test]
    fn test_require_scope() {
        let ctx = AuthContext {
            user_id: "user1".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::SessionRead],
            session_id: None,
            device_id: None,
        };

        assert!(ctx.require_scope(&Scope::SessionRead).is_ok());
        assert!(ctx.require_scope(&Scope::ConfigWrite).is_err());
    }

    #[test]
    fn test_list_keys() {
        let store = AuthStore::new(true);
        store
            .generate_api_key("user1", default_user_scopes(), "key1")
            .unwrap();
        store
            .generate_api_key("user2", admin_scopes(), "key2")
            .unwrap();

        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_scheduler_scopes() {
        let ctx = AuthContext {
            user_id: "user1".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::SchedulerRead],
            session_id: None,
            device_id: None,
        };

        assert!(ctx.has_scope(&Scope::SchedulerRead));
        assert!(!ctx.has_scope(&Scope::SchedulerWrite));
        assert!(ctx.require_scope(&Scope::SchedulerRead).is_ok());
        assert!(ctx.require_scope(&Scope::SchedulerWrite).is_err());

        // Admin should have scheduler scopes
        let admin_ctx = AuthContext {
            user_id: "admin".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::Admin],
            session_id: None,
            device_id: None,
        };
        assert!(admin_ctx.has_scope(&Scope::SchedulerRead));
        assert!(admin_ctx.has_scope(&Scope::SchedulerWrite));
    }

    #[test]
    fn test_default_user_scopes_include_scheduler_read() {
        let scopes = default_user_scopes();
        assert!(scopes.contains(&Scope::SchedulerRead));
        assert!(!scopes.contains(&Scope::SchedulerWrite));
    }

    #[test]
    fn test_active_key_count() {
        let store = AuthStore::new(true);
        let (_, hash) = store
            .generate_api_key("user1", default_user_scopes(), "key1")
            .unwrap();
        store
            .generate_api_key("user2", admin_scopes(), "key2")
            .unwrap();

        assert_eq!(store.active_key_count(), 2);

        store.revoke_key(&hash).unwrap();
        assert_eq!(store.active_key_count(), 1);
    }
}
