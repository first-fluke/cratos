//! Cratos Crypto — E2E encryption for sessions.
//!
//! Provides AES-256-GCM encryption with X25519 key exchange,
//! following the Happy Coder zero-knowledge pattern:
//! - Each session gets a unique symmetric key via X25519 + HKDF
//! - Every message gets a fresh random nonce (no reuse)
//! - Server stores only encrypted blobs (zero-knowledge)
//! - All keys implement `Zeroize` for automatic memory cleanup

#![forbid(unsafe_code)]

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Error types for crypto operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// Encryption failed
    EncryptionFailed,
    /// Decryption failed (wrong key, tampered data, or invalid nonce)
    DecryptionFailed,
    /// Invalid data format
    InvalidFormat(String),
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EncryptionFailed => write!(f, "encryption failed"),
            Self::DecryptionFailed => write!(f, "decryption failed"),
            Self::InvalidFormat(msg) => write!(f, "invalid format: {}", msg),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Result type for crypto operations.
pub type Result<T> = std::result::Result<T, CryptoError>;

/// Encrypted data bundle.
///
/// Contains everything needed to decrypt (except the key):
/// version, nonce, and ciphertext with GCM auth tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Format version (currently 1)
    pub version: u8,
    /// 12-byte nonce (GCM standard)
    pub nonce: [u8; 12],
    /// Ciphertext including GCM authentication tag (16 bytes appended)
    pub ciphertext: Vec<u8>,
}

/// Per-session cipher using AES-256-GCM.
///
/// The key is derived from an X25519 key exchange using HKDF.
/// Implements `Zeroize` + `ZeroizeOnDrop` for automatic key cleanup.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SessionCipher {
    key: [u8; 32],
}

impl SessionCipher {
    /// Create a cipher from a raw 256-bit key.
    pub fn from_key(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// Create a cipher via X25519 key exchange + HKDF key derivation.
    ///
    /// Both sides perform this with their secret and the other's public key
    /// to arrive at the same shared secret.
    pub fn from_key_exchange(our_secret: &StaticSecret, their_public: &PublicKey) -> Self {
        let shared_secret = our_secret.diffie_hellman(their_public);

        // Derive a 256-bit key using HKDF with a fixed info string
        let hkdf = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
        let mut key = [0u8; 32];
        hkdf.expand(b"cratos-session-e2e-v1", &mut key)
            .expect("HKDF expand should never fail with 32-byte output");

        Self { key }
    }

    /// Encrypt plaintext with a fresh random nonce.
    ///
    /// Each call generates a unique nonce, so encrypting the same plaintext
    /// twice produces different ciphertext (semantic security).
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key).map_err(|_| CryptoError::EncryptionFailed)?;

        // Generate fresh random nonce
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::EncryptionFailed)?;

        Ok(EncryptedData {
            version: 1,
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Decrypt an encrypted data bundle.
    pub fn decrypt(&self, data: &EncryptedData) -> Result<Vec<u8>> {
        if data.version != 1 {
            return Err(CryptoError::InvalidFormat(format!(
                "unsupported version: {}",
                data.version
            )));
        }

        let cipher =
            Aes256Gcm::new_from_slice(&self.key).map_err(|_| CryptoError::DecryptionFailed)?;
        let nonce = Nonce::from_slice(&data.nonce);

        cipher
            .decrypt(nonce, data.ciphertext.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)
    }
}

impl std::fmt::Debug for SessionCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCipher")
            .field("key", &"[REDACTED]")
            .finish()
    }
}

/// Generate a new X25519 keypair for key exchange.
///
/// Returns `(secret, public)`. The secret should be kept private,
/// the public key should be sent to the other party.
pub fn generate_keypair() -> (StaticSecret, PublicKey) {
    let secret = StaticSecret::random_from_rng(rand::thread_rng());
    let public = PublicKey::from(&secret);
    (secret, public)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [42u8; 32];
        let cipher = SessionCipher::from_key(key);

        let plaintext = b"Hello, Cratos!";
        let encrypted = cipher.encrypt(plaintext).unwrap();

        assert_eq!(encrypted.version, 1);
        assert_ne!(&encrypted.ciphertext[..], plaintext); // Actually encrypted

        let decrypted = cipher.decrypt(&encrypted).unwrap();
        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_different_nonces() {
        let cipher = SessionCipher::from_key([42u8; 32]);
        let plaintext = b"same message";

        let enc1 = cipher.encrypt(plaintext).unwrap();
        let enc2 = cipher.encrypt(plaintext).unwrap();

        // Same plaintext → different nonce → different ciphertext
        assert_ne!(enc1.nonce, enc2.nonce);
        assert_ne!(enc1.ciphertext, enc2.ciphertext);

        // Both decrypt correctly
        assert_eq!(cipher.decrypt(&enc1).unwrap(), plaintext);
        assert_eq!(cipher.decrypt(&enc2).unwrap(), plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let cipher1 = SessionCipher::from_key([1u8; 32]);
        let cipher2 = SessionCipher::from_key([2u8; 32]);

        let encrypted = cipher1.encrypt(b"secret").unwrap();

        // Wrong key → decryption fails (GCM authentication)
        let result = cipher2.decrypt(&encrypted);
        assert_eq!(result, Err(CryptoError::DecryptionFailed));
    }

    #[test]
    fn test_tampered_data_fails() {
        let cipher = SessionCipher::from_key([42u8; 32]);
        let mut encrypted = cipher.encrypt(b"original").unwrap();

        // Tamper with ciphertext
        if let Some(byte) = encrypted.ciphertext.first_mut() {
            *byte ^= 0xFF;
        }

        let result = cipher.decrypt(&encrypted);
        assert_eq!(result, Err(CryptoError::DecryptionFailed));
    }

    #[test]
    fn test_key_exchange() {
        // Simulate two parties
        let (alice_secret, alice_public) = generate_keypair();
        let (bob_secret, bob_public) = generate_keypair();

        // Both derive the same session key
        let alice_cipher = SessionCipher::from_key_exchange(&alice_secret, &bob_public);
        let bob_cipher = SessionCipher::from_key_exchange(&bob_secret, &alice_public);

        // Alice encrypts, Bob decrypts
        let message = b"Hello from Alice!";
        let encrypted = alice_cipher.encrypt(message).unwrap();
        let decrypted = bob_cipher.decrypt(&encrypted).unwrap();
        assert_eq!(&decrypted, message);

        // Bob encrypts, Alice decrypts
        let response = b"Hello from Bob!";
        let encrypted = bob_cipher.encrypt(response).unwrap();
        let decrypted = alice_cipher.decrypt(&encrypted).unwrap();
        assert_eq!(&decrypted, response);
    }

    #[test]
    fn test_empty_plaintext() {
        let cipher = SessionCipher::from_key([42u8; 32]);
        let encrypted = cipher.encrypt(b"").unwrap();
        let decrypted = cipher.decrypt(&encrypted).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_large_plaintext() {
        let cipher = SessionCipher::from_key([42u8; 32]);
        let large = vec![0xABu8; 1_000_000]; // 1 MB
        let encrypted = cipher.encrypt(&large).unwrap();
        let decrypted = cipher.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, large);
    }

    #[test]
    fn test_invalid_version() {
        let cipher = SessionCipher::from_key([42u8; 32]);
        let data = EncryptedData {
            version: 99,
            nonce: [0u8; 12],
            ciphertext: vec![1, 2, 3],
        };
        let result = cipher.decrypt(&data);
        assert!(matches!(result, Err(CryptoError::InvalidFormat(_))));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let cipher = SessionCipher::from_key([42u8; 32]);
        let encrypted = cipher.encrypt(b"test").unwrap();

        let json = serde_json::to_string(&encrypted).unwrap();
        let parsed: EncryptedData = serde_json::from_str(&json).unwrap();

        let decrypted = cipher.decrypt(&parsed).unwrap();
        assert_eq!(&decrypted, b"test");
    }

    #[test]
    fn test_debug_redacts_key() {
        let cipher = SessionCipher::from_key([42u8; 32]);
        let debug = format!("{:?}", cipher);
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("42"));
    }
}
