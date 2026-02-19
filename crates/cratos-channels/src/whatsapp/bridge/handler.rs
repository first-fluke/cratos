use super::adapter::WhatsAppAdapter;
use super::types::WhatsAppWebhookMessage;
use crate::error::Result;
use crate::message::{ChannelAdapter, OutgoingMessage};
use crate::util::{mask_for_logging, WHATSAPP_MESSAGE_LIMIT};
use cratos_core::{Orchestrator, OrchestratorInput};
use std::sync::Arc;
use tracing::{error, info, instrument};

/// WhatsApp event handler (Baileys bridge)
pub struct WhatsAppHandler {
    adapter: Arc<WhatsAppAdapter>,
    orchestrator: Arc<Orchestrator>,
}

impl WhatsAppHandler {
    pub fn new(adapter: Arc<WhatsAppAdapter>, orchestrator: Arc<Orchestrator>) -> Self {
        Self {
            adapter,
            orchestrator,
        }
    }

    /// Handle incoming webhook message
    #[instrument(skip(self, msg))]
    pub async fn handle_webhook(
        &self,
        msg: WhatsAppWebhookMessage,
    ) -> Result<()> {
        let Some(normalized) = self.adapter.normalize_webhook_message(&msg) else {
            return Ok(());
        };

        info!(
            from = %normalized.user_id,
            text = %mask_for_logging(&normalized.text),
            "Received WhatsApp message"
        );

        // Send typing indicator
        let _ = self.adapter.send_typing(&msg.from).await;

        // Process with orchestrator
        let input = OrchestratorInput::new(
            "whatsapp",
            &normalized.channel_id,
            &normalized.user_id,
            &normalized.text,
        );

        match self.orchestrator.process(input).await {
            Ok(result) => {
                let response_text = if result.response.is_empty() {
                    "Done.".to_string()
                } else {
                    result.response
                };

                // WhatsApp has no strict message length limit, but split long messages
                if response_text.len() > WHATSAPP_MESSAGE_LIMIT {
                    for chunk in response_text.as_bytes().chunks(WHATSAPP_MESSAGE_LIMIT) {
                        if let Ok(text) = std::str::from_utf8(chunk) {
                            let _ = self.adapter
                                .send_message(&msg.from, OutgoingMessage::text(text))
                                .await;
                        }
                    }
                } else {
                    let _ = self.adapter
                        .send_message(&msg.from, OutgoingMessage::text(response_text))
                        .await;
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to process WhatsApp message");
                let _ = self.adapter
                    .send_message(
                        &msg.from,
                        OutgoingMessage::text("Sorry, I encountered an error. Please try again."),
                    )
                    .await;
            }
        }

        Ok(())
    }
}
