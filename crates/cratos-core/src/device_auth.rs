//! Ed25519 device authentication utilities.
//!
//! Implements the Happy Coder challenge-response pattern:
//! 1. Server generates a random challenge
//! 2. Device signs the challenge with its Ed25519 private key
//! 3. Server verifies the signature using the stored public key
//!
//! This ensures that only the device holding the private key can authenticate,
//! without ever transmitting the private key.

#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Errors from device authentication operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceAuthError {
    /// Public key is malformed (not 32 bytes)
    InvalidPublicKey,
    /// Signature is malformed (not 64 bytes)
    InvalidSignature,
    /// Signature verification failed
    VerificationFailed,
    /// Challenge not found or expired
    ChallengeNotFound,
    /// Challenge has expired
    ChallengeExpired,
    /// Device not registered
    DeviceNotFound,
}

impl std::fmt::Display for DeviceAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPublicKey => write!(f, "invalid public key format"),
            Self::InvalidSignature => write!(f, "invalid signature format"),
            Self::VerificationFailed => write!(f, "signature verification failed"),
            Self::ChallengeNotFound => write!(f, "challenge not found"),
            Self::ChallengeExpired => write!(f, "challenge expired"),
            Self::DeviceNotFound => write!(f, "device not registered"),
        }
    }
}

impl std::error::Error for DeviceAuthError {}

/// Generate a 32-byte random challenge using CSPRNG.
pub fn generate_challenge() -> [u8; 32] {
    let mut challenge = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut challenge);
    challenge
}

/// Verify an Ed25519 signature.
///
/// - `public_key`: 32-byte Ed25519 public key
/// - `message`: the data that was signed (typically a challenge)
/// - `signature`: 64-byte Ed25519 signature
pub fn verify_signature(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), DeviceAuthError> {
    let vk_bytes: [u8; 32] = public_key
        .try_into()
        .map_err(|_| DeviceAuthError::InvalidPublicKey)?;
    let verifying_key =
        VerifyingKey::from_bytes(&vk_bytes).map_err(|_| DeviceAuthError::InvalidPublicKey)?;

    let sig_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| DeviceAuthError::InvalidSignature)?;
    let sig = Signature::from_bytes(&sig_bytes);

    verifying_key
        .verify(message, &sig)
        .map_err(|_| DeviceAuthError::VerificationFailed)
}

/// Generate a new Ed25519 keypair (utility for node agents).
///
/// Returns `(signing_key, verifying_key)`.
pub fn generate_device_keypair() -> (SigningKey, VerifyingKey) {
    let signing_key = SigningKey::generate(&mut rand::thread_rng());
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

/// Sign a challenge with a signing key (node side).
pub fn sign_challenge(signing_key: &SigningKey, challenge: &[u8]) -> Vec<u8> {
    use ed25519_dalek::Signer;
    let sig = signing_key.sign(challenge);
    sig.to_bytes().to_vec()
}

/// Pending challenge with TTL.
struct PendingChallenge {
    challenge: [u8; 32],
    created_at: DateTime<Utc>,
}

/// Manages pending challenges with TTL expiration.
///
/// Challenges are keyed by device_id and expire after `ttl_secs` (default 60s).
pub struct ChallengeStore {
    challenges: RwLock<HashMap<String, PendingChallenge>>,
    ttl_secs: i64,
}

impl ChallengeStore {
    /// Create a new challenge store with default TTL (60 seconds).
    pub fn new() -> Self {
        Self {
            challenges: RwLock::new(HashMap::new()),
            ttl_secs: 60,
        }
    }

    /// Create with custom TTL.
    pub fn with_ttl_secs(ttl_secs: i64) -> Self {
        Self {
            challenges: RwLock::new(HashMap::new()),
            ttl_secs,
        }
    }

    /// Issue a new challenge for a device.
    ///
    /// Replaces any existing challenge for the same device.
    pub async fn issue(&self, device_id: &str) -> [u8; 32] {
        let challenge = generate_challenge();
        let pending = PendingChallenge {
            challenge,
            created_at: Utc::now(),
        };
        let mut store = self.challenges.write().await;
        store.insert(device_id.to_string(), pending);
        challenge
    }

    /// Verify a challenge response.
    ///
    /// Consumes the challenge (one-time use) and checks TTL.
    pub async fn verify(
        &self,
        device_id: &str,
        challenge: &[u8; 32],
    ) -> Result<(), DeviceAuthError> {
        let mut store = self.challenges.write().await;
        let pending = store
            .remove(device_id)
            .ok_or(DeviceAuthError::ChallengeNotFound)?;

        // Check TTL (use num_milliseconds for sub-second precision)
        let elapsed_ms = (Utc::now() - pending.created_at).num_milliseconds();
        let ttl_ms = self.ttl_secs * 1000;
        if elapsed_ms >= ttl_ms {
            return Err(DeviceAuthError::ChallengeExpired);
        }

        // Check challenge matches
        if pending.challenge != *challenge {
            return Err(DeviceAuthError::ChallengeNotFound);
        }

        Ok(())
    }

    /// Clean up expired challenges.
    pub async fn cleanup(&self) -> usize {
        let cutoff = Utc::now() - chrono::Duration::seconds(self.ttl_secs);
        let mut store = self.challenges.write().await;
        let before = store.len();
        store.retain(|_, v| v.created_at > cutoff);
        before - store.len()
    }
}

impl Default for ChallengeStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_keypair_and_verify() {
        let (signing_key, verifying_key) = generate_device_keypair();
        let challenge = generate_challenge();
        let signature = sign_challenge(&signing_key, &challenge);

        let result = verify_signature(verifying_key.as_bytes(), &challenge, &signature);
        assert!(result.is_ok());
    }

    #[test]
    fn test_wrong_signature_fails() {
        let (signing_key, _) = generate_device_keypair();
        let (_, other_verifying_key) = generate_device_keypair();

        let challenge = generate_challenge();
        let signature = sign_challenge(&signing_key, &challenge);

        // Wrong public key
        let result = verify_signature(other_verifying_key.as_bytes(), &challenge, &signature);
        assert_eq!(result, Err(DeviceAuthError::VerificationFailed));
    }

    #[test]
    fn test_wrong_message_fails() {
        let (signing_key, verifying_key) = generate_device_keypair();
        let challenge = generate_challenge();
        let signature = sign_challenge(&signing_key, &challenge);

        // Different message
        let other_challenge = generate_challenge();
        let result = verify_signature(verifying_key.as_bytes(), &other_challenge, &signature);
        assert_eq!(result, Err(DeviceAuthError::VerificationFailed));
    }

    #[test]
    fn test_invalid_public_key() {
        let result = verify_signature(&[0u8; 16], &[0u8; 32], &[0u8; 64]);
        assert_eq!(result, Err(DeviceAuthError::InvalidPublicKey));
    }

    #[test]
    fn test_invalid_signature_length() {
        let (_, verifying_key) = generate_device_keypair();
        let result = verify_signature(
            verifying_key.as_bytes(),
            &[0u8; 32],
            &[0u8; 32], // Wrong length
        );
        assert_eq!(result, Err(DeviceAuthError::InvalidSignature));
    }

    #[tokio::test]
    async fn test_challenge_store_issue_and_verify() {
        let store = ChallengeStore::new();
        let challenge = store.issue("device-1").await;

        let result = store.verify("device-1", &challenge).await;
        assert!(result.is_ok());

        // Second verify should fail (one-time use)
        let result = store.verify("device-1", &challenge).await;
        assert_eq!(result, Err(DeviceAuthError::ChallengeNotFound));
    }

    #[tokio::test]
    async fn test_challenge_store_expired() {
        let store = ChallengeStore::with_ttl_secs(0); // Immediate expiry
        let challenge = store.issue("device-1").await;

        // Sleep to ensure expiry
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let result = store.verify("device-1", &challenge).await;
        assert_eq!(result, Err(DeviceAuthError::ChallengeExpired));
    }

    #[tokio::test]
    async fn test_challenge_store_wrong_device() {
        let store = ChallengeStore::new();
        let challenge = store.issue("device-1").await;

        let result = store.verify("device-2", &challenge).await;
        assert_eq!(result, Err(DeviceAuthError::ChallengeNotFound));
    }

    #[tokio::test]
    async fn test_challenge_store_cleanup() {
        let store = ChallengeStore::with_ttl_secs(0);
        store.issue("d1").await;
        store.issue("d2").await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let cleaned = store.cleanup().await;
        assert_eq!(cleaned, 2);
    }

    #[test]
    fn test_full_challenge_response_flow() {
        // Simulate the full Happy Coder pattern:
        // 1. Node generates keypair
        let (node_signing_key, node_verifying_key) = generate_device_keypair();

        // 2. Server generates challenge
        let challenge = generate_challenge();

        // 3. Node signs challenge
        let signature = sign_challenge(&node_signing_key, &challenge);

        // 4. Server verifies with stored public key
        let result = verify_signature(node_verifying_key.as_bytes(), &challenge, &signature);
        assert!(result.is_ok());
    }
}
