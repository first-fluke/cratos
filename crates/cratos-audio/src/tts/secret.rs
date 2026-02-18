use std::fmt;

/// A wrapper type for secrets that prevents accidental logging.
/// This replaces the `secrecy` crate to avoid build issues while maintaining security.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret(***)")
    }
}

// Implement From<String> for convenience
impl From<String> for SecretString {
    fn from(s: String) -> Self {
        Self(s)
    }
}
