//! Gateway WebSocket wire protocol.
//!
//! Defines the frame format and error types for the Gateway WS endpoint.
//! Based on OpenClaw's request/response/event framing pattern.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Gateway wire frame — all messages on the WebSocket are one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum GatewayFrame {
    /// Client → Server: method invocation
    Request {
        /// Unique request ID (client-generated)
        id: String,
        /// Method name (e.g. "connect", "chat.send")
        method: String,
        /// Method parameters
        #[serde(default)]
        params: Value,
    },
    /// Server → Client: response to a request
    Response {
        /// Matches the request ID
        id: String,
        /// Successful result (mutually exclusive with error)
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        /// Error (mutually exclusive with result)
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<GatewayError>,
    },
    /// Server → Client: unsolicited event
    Event {
        /// Event name
        event: String,
        /// Event payload
        data: Value,
    },
}

impl GatewayFrame {
    /// Create a success response for a given request ID.
    pub fn ok(id: impl Into<String>, result: Value) -> Self {
        Self::Response {
            id: id.into(),
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response for a given request ID.
    pub fn err(id: impl Into<String>, error: GatewayError) -> Self {
        Self::Response {
            id: id.into(),
            result: None,
            error: Some(error),
        }
    }

    /// Create an event frame.
    pub fn event(name: impl Into<String>, data: Value) -> Self {
        Self::Event {
            event: name.into(),
            data,
        }
    }
}

/// Structured error in a Response frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayError {
    /// Machine-readable error code
    pub code: GatewayErrorCode,
    /// Human-readable message
    pub message: String,
}

impl GatewayError {
    pub fn new(code: GatewayErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Error codes for the Gateway protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GatewayErrorCode {
    /// Authentication failed or missing
    Unauthorized,
    /// Insufficient scopes
    Forbidden,
    /// Must call "connect" first
    NotConnected,
    /// Unknown method
    UnknownMethod,
    /// Invalid parameters
    InvalidParams,
    /// Resource not found
    NotFound,
    /// Rate limit exceeded
    RateLimited,
    /// Internal server error
    InternalError,
}

/// Parameters for the `connect` method.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ConnectParams {
    /// Authentication token (Bearer token or API key)
    pub token: String,
    /// Client info
    #[serde(default)]
    pub client: ClientInfo,
    /// Protocol version requested
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
    /// Client role: "operator" (default) or "browser" (extension)
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "operator".to_string()
}

fn default_protocol_version() -> u32 {
    1
}

/// Client metadata sent during connect.
#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
pub struct ClientInfo {
    /// Client name (e.g. "cratos-web", "cratos-tui")
    #[serde(default)]
    pub name: String,
    /// Client version
    #[serde(default)]
    pub version: String,
}

/// Successful connect response data.
#[derive(Debug, Serialize)]
pub struct ConnectResult {
    /// Assigned session ID
    pub session_id: Uuid,
    /// Scopes granted
    pub scopes: Vec<String>,
    /// Protocol version agreed upon
    pub protocol_version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let frame = GatewayFrame::Request {
            id: "1".to_string(),
            method: "chat.send".to_string(),
            params: serde_json::json!({"text": "hello"}),
        };
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"frame\":\"request\""));
        assert!(json.contains("\"method\":\"chat.send\""));

        let parsed: GatewayFrame = serde_json::from_str(&json).unwrap();
        match parsed {
            GatewayFrame::Request { id, method, .. } => {
                assert_eq!(id, "1");
                assert_eq!(method, "chat.send");
            }
            _ => panic!("expected Request"),
        }
    }

    #[test]
    fn test_response_ok() {
        let frame = GatewayFrame::ok("1", serde_json::json!({"status": "ok"}));
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"frame\":\"response\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_response_err() {
        let frame = GatewayFrame::err(
            "2",
            GatewayError::new(GatewayErrorCode::Unauthorized, "bad token"),
        );
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("UNAUTHORIZED"));
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_event_frame() {
        let frame = GatewayFrame::event("execution_started", serde_json::json!({"id": "abc"}));
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"frame\":\"event\""));
        assert!(json.contains("\"event\":\"execution_started\""));
    }

    #[test]
    fn test_connect_params_deserialization() {
        let json = r#"{"token": "cratos_abc", "client": {"name": "web", "version": "1.0"}, "protocol_version": 1}"#;
        let params: ConnectParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.token, "cratos_abc");
        assert_eq!(params.client.name, "web");
        assert_eq!(params.protocol_version, 1);
    }

    #[test]
    fn test_connect_params_minimal() {
        let json = r#"{"token": "key123"}"#;
        let params: ConnectParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.token, "key123");
        assert_eq!(params.protocol_version, 1);
    }

    #[test]
    fn test_error_codes_roundtrip() {
        let codes = vec![
            GatewayErrorCode::Unauthorized,
            GatewayErrorCode::Forbidden,
            GatewayErrorCode::NotConnected,
            GatewayErrorCode::UnknownMethod,
            GatewayErrorCode::InvalidParams,
            GatewayErrorCode::NotFound,
            GatewayErrorCode::RateLimited,
            GatewayErrorCode::InternalError,
        ];
        for code in codes {
            let json = serde_json::to_string(&code).unwrap();
            let parsed: GatewayErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, code);
        }
    }
}
