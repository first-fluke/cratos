use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Verify an Ed25519 signature.
///
/// Returns Ok(true) if valid, Ok(false) if invalid but well-formed, Err if malformed.
pub fn verify_signature(public_key: &str, message: &str, signature: &str) -> Result<bool, String> {
    let public_bytes = URL_SAFE_NO_PAD
        .decode(public_key)
        .map_err(|e| e.to_string())?;
    let signature_bytes = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|e| e.to_string())?;

    let public = VerifyingKey::from_bytes(
        public_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid public key length")?,
    )
    .map_err(|e| e.to_string())?;

    let sig = Signature::from_bytes(
        signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid signature length")?,
    );

    public
        .verify(message.as_bytes(), &sig)
        .map(|_| true)
        .map_err(|e| e.to_string())
}
