//! Secure string implementation with cryptographic memory wiping

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A string that is cryptographically cleared from memory when dropped
///
/// Uses the `zeroize` crate for secure memory wiping, which:
/// - Overwrites memory with zeros before deallocation
/// - Prevents compiler optimizations from removing the zeroing
/// - Works without unsafe code
///
/// # Security
///
/// - Value is zeroized on drop (via `ZeroizeOnDrop`)
/// - Debug and Display implementations redact the value
/// - Clone creates a new secure copy
///
/// # Example
///
/// ```
/// use cratos_core::credentials::SecureString;
///
/// let secret = SecureString::new("api-key-12345");
/// assert_eq!(secret.expose(), "api-key-12345");
///
/// // Debug output is redacted
/// let debug = format!("{:?}", secret);
/// assert!(!debug.contains("api-key"));
/// ```
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecureString {
    inner: String,
}

impl SecureString {
    /// Create a new secure string
    ///
    /// The value will be cryptographically zeroed when the SecureString is dropped.
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self { inner: s.into() }
    }

    /// Temporarily expose the string value
    ///
    /// # Security Warning
    ///
    /// The returned reference should not be stored or cloned unnecessarily.
    /// Prefer using this in limited scopes.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.inner
    }

    /// Get the length of the secret
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the secret is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Explicitly zeroize the string and replace with empty
    ///
    /// This can be called before drop if you want to clear the secret early.
    pub fn clear(&mut self) {
        self.inner.zeroize();
    }
}

impl std::fmt::Debug for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecureString([REDACTED, {} bytes])", self.inner.len())
    }
}

impl std::fmt::Display for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

// Prevent accidental comparison that might leak timing info
impl PartialEq for SecureString {
    fn eq(&self, other: &Self) -> bool {
        // Use constant-time comparison to prevent timing attacks
        self.inner.as_bytes().ct_eq(other.inner.as_bytes()).into()
    }
}

impl Eq for SecureString {}
