//! Gateway browser.* method handlers.
//!
//! Routes browser control requests through the BrowserRelay to the connected
//! Chrome extension, or returns an error if no extension is connected.

use serde_json::Value;

use crate::websocket::gateway::browser_relay::SharedBrowserRelay;
use crate::websocket::protocol::{GatewayError, GatewayErrorCode, GatewayFrame};

/// Handle a `browser.*` gateway method.
pub(crate) async fn handle(
    id: &str,
    method: &str,
    params: Value,
    relay: &SharedBrowserRelay,
) -> GatewayFrame {
    match method {
        "browser.status" => {
            let connected = relay.is_connected().await;
            GatewayFrame::ok(id, serde_json::json!({ "connected": connected }))
        }
        "browser.tabs" | "browser.screenshot" | "browser.action" | "browser.open"
        | "browser.navigate" | "browser.context" => {
            // Relay to the connected extension
            match relay.send_request(method, params).await {
                Ok(result) => GatewayFrame::ok(id, result),
                Err(msg) => {
                    GatewayFrame::err(id, GatewayError::new(GatewayErrorCode::InternalError, msg))
                }
            }
        }
        _ => GatewayFrame::err(
            id,
            GatewayError::new(
                GatewayErrorCode::UnknownMethod,
                format!("Unknown browser method: {}", method),
            ),
        ),
    }
}
