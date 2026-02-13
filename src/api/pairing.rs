//! Device pairing REST API endpoints.
//!
//! - `POST /api/v1/pair/start` — Generate a 6-digit PIN
//! - `POST /api/v1/pair/verify` — Verify PIN and register device
//! - `GET /api/v1/pair/devices` — List paired devices
//! - `DELETE /api/v1/pair/devices/:id` — Unpair a device

use axum::extract::{Extension, Path};
use axum::response::Json;
use axum::routing::{delete, get, post};
use axum::Router;
use cratos_core::device_auth::ChallengeStore;
use cratos_core::pairing::PairingManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Start pairing response
#[derive(Debug, Serialize)]
struct StartPairingResponse {
    pin: String,
    expires_in_secs: u64,
}

/// Verify pairing request
#[derive(Debug, Deserialize)]
struct VerifyPairingRequest {
    pin: String,
    device_name: String,
    /// Base64-encoded Ed25519 public key (32 bytes)
    public_key: String,
}

/// Verify pairing response
#[derive(Debug, Serialize)]
struct VerifyPairingResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Paired device info
#[derive(Debug, Serialize)]
struct DeviceInfo {
    device_id: String,
    device_name: String,
    paired_at: String,
}

/// Start a pairing session
async fn start_pairing(
    Extension(mgr): Extension<Arc<PairingManager>>,
) -> Json<StartPairingResponse> {
    let pin = mgr.start_pairing().await;
    Json(StartPairingResponse {
        pin,
        expires_in_secs: 300,
    })
}

/// Verify PIN and register device
async fn verify_pairing(
    Extension(mgr): Extension<Arc<PairingManager>>,
    Json(req): Json<VerifyPairingRequest>,
) -> Json<VerifyPairingResponse> {
    use base64::Engine;

    let public_key = match base64::engine::general_purpose::STANDARD.decode(&req.public_key) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Json(VerifyPairingResponse {
                success: false,
                device_id: None,
                error: Some("Invalid base64 public key".to_string()),
            });
        }
    };

    let result = mgr
        .verify_pin(&req.pin, &req.device_name, &public_key)
        .await;

    Json(VerifyPairingResponse {
        success: result.success,
        device_id: result.device_id,
        error: result.error,
    })
}

/// List paired devices
async fn list_devices(Extension(mgr): Extension<Arc<PairingManager>>) -> Json<Vec<DeviceInfo>> {
    let devices = mgr.list_devices().await;
    let infos: Vec<DeviceInfo> = devices
        .iter()
        .map(|d| DeviceInfo {
            device_id: d.device_id.clone(),
            device_name: d.device_name.clone(),
            paired_at: d.paired_at.to_rfc3339(),
        })
        .collect();
    Json(infos)
}

/// Unpair a device
async fn unpair_device(
    Extension(mgr): Extension<Arc<PairingManager>>,
    Path(device_id): Path<String>,
) -> Json<serde_json::Value> {
    let removed = mgr.unpair_device(&device_id).await;
    Json(serde_json::json!({
        "success": removed,
    }))
}

/// Challenge request
#[derive(Debug, Deserialize)]
struct ChallengeRequest {
    device_id: String,
}

/// Challenge response
#[derive(Debug, Serialize)]
struct ChallengeResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    challenge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Authenticate request (verify challenge signature)
#[derive(Debug, Deserialize)]
struct AuthenticateRequest {
    device_id: String,
    /// Base64-encoded challenge bytes
    challenge: String,
    /// Base64-encoded Ed25519 signature
    signature: String,
}

/// Authenticate response
#[derive(Debug, Serialize)]
struct AuthenticateResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Request a challenge for an already-paired device
async fn request_challenge(
    Extension(mgr): Extension<Arc<PairingManager>>,
    Extension(challenge_store): Extension<Arc<ChallengeStore>>,
    Json(req): Json<ChallengeRequest>,
) -> Json<ChallengeResponse> {
    // Check device exists
    if mgr.get_device(&req.device_id).await.is_none() {
        return Json(ChallengeResponse {
            challenge: None,
            error: Some("Device not found".to_string()),
        });
    }

    use base64::Engine;
    let challenge = challenge_store.issue(&req.device_id).await;
    let challenge_b64 = base64::engine::general_purpose::STANDARD.encode(challenge);

    Json(ChallengeResponse {
        challenge: Some(challenge_b64),
        error: None,
    })
}

/// Authenticate a device by verifying its challenge signature
async fn authenticate_device(
    Extension(mgr): Extension<Arc<PairingManager>>,
    Extension(challenge_store): Extension<Arc<ChallengeStore>>,
    Json(req): Json<AuthenticateRequest>,
) -> Json<AuthenticateResponse> {
    use base64::Engine;

    // Decode challenge
    let challenge_bytes = match base64::engine::general_purpose::STANDARD.decode(&req.challenge) {
        Ok(b) => b,
        Err(_) => {
            return Json(AuthenticateResponse {
                success: false,
                token: None,
                expires_in: None,
                error: Some("Invalid base64 challenge".to_string()),
            });
        }
    };

    // Decode signature
    let signature_bytes = match base64::engine::general_purpose::STANDARD.decode(&req.signature) {
        Ok(b) => b,
        Err(_) => {
            return Json(AuthenticateResponse {
                success: false,
                token: None,
                expires_in: None,
                error: Some("Invalid base64 signature".to_string()),
            });
        }
    };

    // Verify challenge exists and is not expired
    let challenge_arr: [u8; 32] = match challenge_bytes.try_into() {
        Ok(arr) => arr,
        Err(_) => {
            return Json(AuthenticateResponse {
                success: false,
                token: None,
                expires_in: None,
                error: Some("Challenge must be 32 bytes".to_string()),
            });
        }
    };

    if let Err(e) = challenge_store.verify(&req.device_id, &challenge_arr).await {
        return Json(AuthenticateResponse {
            success: false,
            token: None,
            expires_in: None,
            error: Some(format!("Challenge verification failed: {}", e)),
        });
    }

    // Get device public key
    let public_key = match mgr.get_device_public_key(&req.device_id).await {
        Ok(pk) => pk,
        Err(e) => {
            return Json(AuthenticateResponse {
                success: false,
                token: None,
                expires_in: None,
                error: Some(format!("Device error: {}", e)),
            });
        }
    };

    // Verify Ed25519 signature
    if let Err(e) =
        cratos_core::device_auth::verify_signature(&public_key, &challenge_arr, &signature_bytes)
    {
        return Json(AuthenticateResponse {
            success: false,
            token: None,
            expires_in: None,
            error: Some(format!("Signature verification failed: {}", e)),
        });
    }

    // Generate a simple session token (UUID-based, sufficient for device auth)
    let token = uuid::Uuid::new_v4().to_string();
    let expires_in = 3600u64; // 1 hour

    Json(AuthenticateResponse {
        success: true,
        token: Some(token),
        expires_in: Some(expires_in),
        error: None,
    })
}

/// Create pairing routes
pub fn pairing_routes() -> Router {
    Router::new()
        .route("/api/v1/pair/start", post(start_pairing))
        .route("/api/v1/pair/verify", post(verify_pairing))
        .route("/api/v1/pair/devices", get(list_devices))
        .route("/api/v1/pair/devices/{id}", delete(unpair_device))
        .route("/api/v1/pair/challenge", post(request_challenge))
        .route("/api/v1/pair/authenticate", post(authenticate_device))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_pairing_response_serialization() {
        let resp = StartPairingResponse {
            pin: "123456".to_string(),
            expires_in_secs: 300,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("123456"));
        assert!(json.contains("300"));
    }

    #[test]
    fn test_verify_request_deserialization() {
        let json = r#"{"pin":"654321","device_name":"my-phone","public_key":"AAAA"}"#;
        let req: VerifyPairingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.pin, "654321");
        assert_eq!(req.device_name, "my-phone");
    }

    #[test]
    fn test_challenge_request_deserialization() {
        let json = r#"{"device_id":"abc-123"}"#;
        let req: ChallengeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.device_id, "abc-123");
    }

    #[test]
    fn test_authenticate_request_deserialization() {
        let json = r#"{"device_id":"abc","challenge":"AAAA","signature":"BBBB"}"#;
        let req: AuthenticateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.device_id, "abc");
        assert_eq!(req.challenge, "AAAA");
        assert_eq!(req.signature, "BBBB");
    }

    #[test]
    fn test_authenticate_response_serialization() {
        let resp = AuthenticateResponse {
            success: true,
            token: Some("tok-123".to_string()),
            expires_in: Some(3600),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("tok-123"));
        assert!(json.contains("3600"));
        assert!(!json.contains("error"));
    }
}
