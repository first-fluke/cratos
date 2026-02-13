//! ACP wire protocol — stdin/stdout JSON lines.
//!
//! ACP messages map 1:1 to Gateway WS frames, enabling IDE tools
//! to communicate with Cratos via stdin/stdout instead of WebSocket.

use crate::websocket::protocol::{GatewayError, GatewayFrame};
use serde::{Deserialize, Serialize};

/// ACP message (stdin/stdout JSON lines).
///
/// Structurally identical to `GatewayFrame` but with `type` tag
/// for JSON-lines parsing convenience.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AcpMessage {
    /// Client → server request
    Request {
        id: String,
        method: String,
        #[serde(default)]
        params: serde_json::Value,
    },
    /// Server → client response
    Response {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<GatewayError>,
    },
    /// Server → client event
    Event {
        event: String,
        data: serde_json::Value,
    },
}

impl From<GatewayFrame> for AcpMessage {
    fn from(frame: GatewayFrame) -> Self {
        match frame {
            GatewayFrame::Request { id, method, params } => {
                AcpMessage::Request { id, method, params }
            }
            GatewayFrame::Response { id, result, error } => {
                AcpMessage::Response { id, result, error }
            }
            GatewayFrame::Event { event, data } => AcpMessage::Event { event, data },
        }
    }
}

impl From<AcpMessage> for GatewayFrame {
    fn from(msg: AcpMessage) -> Self {
        match msg {
            AcpMessage::Request { id, method, params } => {
                GatewayFrame::Request { id, method, params }
            }
            AcpMessage::Response { id, result, error } => {
                GatewayFrame::Response { id, result, error }
            }
            AcpMessage::Event { event, data } => GatewayFrame::Event { event, data },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::protocol::GatewayErrorCode;

    #[test]
    fn test_request_roundtrip() {
        let msg = AcpMessage::Request {
            id: "1".to_string(),
            method: "chat.send".to_string(),
            params: serde_json::json!({"text": "hello"}),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"request\""));

        let parsed: AcpMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            AcpMessage::Request { id, method, params } => {
                assert_eq!(id, "1");
                assert_eq!(method, "chat.send");
                assert_eq!(params["text"], "hello");
            }
            _ => panic!("expected request"),
        }
    }

    #[test]
    fn test_response_roundtrip() {
        let msg = AcpMessage::Response {
            id: "1".to_string(),
            result: Some(serde_json::json!({"ok": true})),
            error: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let parsed: AcpMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            AcpMessage::Response { id, result, error } => {
                assert_eq!(id, "1");
                assert!(result.is_some());
                assert!(error.is_none());
            }
            _ => panic!("expected response"),
        }
    }

    #[test]
    fn test_event_roundtrip() {
        let msg = AcpMessage::Event {
            event: "chat.delta".to_string(),
            data: serde_json::json!({"delta": "hi"}),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"event\""));

        let parsed: AcpMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            AcpMessage::Event { event, data } => {
                assert_eq!(event, "chat.delta");
                assert_eq!(data["delta"], "hi");
            }
            _ => panic!("expected event"),
        }
    }

    #[test]
    fn test_gateway_frame_to_acp_message() {
        let frame = GatewayFrame::ok("1", serde_json::json!({"pong": true}));
        let msg: AcpMessage = frame.into();
        match msg {
            AcpMessage::Response { id, result, error } => {
                assert_eq!(id, "1");
                assert!(result.is_some());
                assert!(error.is_none());
            }
            _ => panic!("expected response"),
        }
    }

    #[test]
    fn test_acp_message_to_gateway_frame() {
        let msg = AcpMessage::Request {
            id: "2".to_string(),
            method: "ping".to_string(),
            params: serde_json::json!({}),
        };
        let frame: GatewayFrame = msg.into();
        match frame {
            GatewayFrame::Request { id, method, .. } => {
                assert_eq!(id, "2");
                assert_eq!(method, "ping");
            }
            _ => panic!("expected request"),
        }
    }

    #[test]
    fn test_error_response_conversion() {
        let frame = GatewayFrame::err(
            "3",
            GatewayError::new(GatewayErrorCode::Forbidden, "Not allowed"),
        );
        let msg: AcpMessage = frame.into();
        match msg {
            AcpMessage::Response { id, error, .. } => {
                assert_eq!(id, "3");
                assert!(error.is_some());
                assert_eq!(error.unwrap().code, GatewayErrorCode::Forbidden);
            }
            _ => panic!("expected error response"),
        }
    }
}
