//! Webhook handlers for external messaging services

use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use cratos_channels::{WhatsAppBusinessAdapter, WhatsAppBusinessWebhook};
use cratos_core::Orchestrator;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, info, warn};

/// WhatsApp Business webhook verification query
#[derive(Debug, Deserialize)]
pub struct WebhookVerifyQuery {
    #[serde(rename = "hub.mode")]
    pub mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    pub verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    pub challenge: Option<String>,
}

/// Verify WhatsApp Business webhook (GET)
///
/// Meta sends this request during webhook setup to verify ownership.
async fn whatsapp_business_verify(
    Query(query): Query<WebhookVerifyQuery>,
    Extension(adapter): Extension<Arc<WhatsAppBusinessAdapter>>,
) -> impl IntoResponse {
    let mode = query.mode.as_deref().unwrap_or("");
    let token = query.verify_token.as_deref().unwrap_or("");
    let challenge = query.challenge.as_deref().unwrap_or("");

    match adapter.verify_webhook(mode, token, challenge) {
        Some(c) => {
            info!("WhatsApp Business webhook verified");
            c.into_response()
        }
        None => {
            warn!("WhatsApp Business webhook verification failed");
            (StatusCode::FORBIDDEN, "Verification failed").into_response()
        }
    }
}

/// Handle WhatsApp Business webhook (POST)
///
/// Receives incoming messages and status updates from Meta.
async fn whatsapp_business_webhook(
    Extension(orchestrator): Extension<Arc<Orchestrator>>,
    Extension(adapter): Extension<Arc<WhatsAppBusinessAdapter>>,
    Json(payload): Json<WhatsAppBusinessWebhook>,
) -> StatusCode {
    info!("Received WhatsApp Business webhook");

    // Process the webhook
    if let Err(e) = adapter.handle_webhook(orchestrator, payload).await {
        error!(error = %e, "Failed to process WhatsApp Business webhook");
    }

    // Always return 200 to avoid retries from Meta
    StatusCode::OK
}

/// Create webhook routes
pub fn webhooks_routes() -> Router {
    Router::new()
        .route(
            "/api/v1/webhooks/whatsapp-business",
            get(whatsapp_business_verify).post(whatsapp_business_webhook),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_query_deserialize() {
        let query = "hub.mode=subscribe&hub.verify_token=test&hub.challenge=abc123";
        let parsed: WebhookVerifyQuery = serde_urlencoded::from_str(query).unwrap();
        assert_eq!(parsed.mode.as_deref(), Some("subscribe"));
        assert_eq!(parsed.challenge.as_deref(), Some("abc123"));
    }
}
