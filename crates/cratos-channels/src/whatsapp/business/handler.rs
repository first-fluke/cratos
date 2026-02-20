use super::adapter::WhatsAppBusinessAdapter;
use super::types::WhatsAppBusinessWebhook;
use crate::error::Result;
use crate::message::{ChannelAdapter, OutgoingMessage};
use crate::util::{mask_for_logging, WHATSAPP_MESSAGE_LIMIT};
use cratos_core::{Orchestrator, OrchestratorInput};
use std::sync::Arc;
use tracing::{error, info, instrument};

/// WhatsApp Business event handler
pub struct WhatsAppBusinessHandler {
    adapter: Arc<WhatsAppBusinessAdapter>,
    orchestrator: Arc<Orchestrator>,
}

impl WhatsAppBusinessHandler {
    pub fn new(adapter: Arc<WhatsAppBusinessAdapter>, orchestrator: Arc<Orchestrator>) -> Self {
        Self {
            adapter,
            orchestrator,
        }
    }

    /// Handle incoming webhook payload
    #[instrument(skip(self, webhook))]
    pub async fn handle_webhook(&self, webhook: WhatsAppBusinessWebhook) -> Result<()> {
        let messages = self.adapter.extract_messages(&webhook);

        for (sender_name, msg) in messages {
            let Some(normalized) = self.adapter.normalize_webhook_message(&sender_name, &msg)
            else {
                continue;
            };

            info!(
                from = %normalized.user_id,
                text = %mask_for_logging(&normalized.text),
                "Received WhatsApp Business message"
            );

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

                    // WhatsApp Business API has a message character limit
                    if response_text.len() > WHATSAPP_MESSAGE_LIMIT {
                        for chunk in response_text.as_bytes().chunks(WHATSAPP_MESSAGE_LIMIT) {
                            if let Ok(text) = std::str::from_utf8(chunk) {
                                let _ = self
                                    .adapter
                                    .send_message(&msg.from, OutgoingMessage::text(text))
                                    .await;
                            }
                        }
                    } else {
                        let _ = self
                            .adapter
                            .send_message(&msg.from, OutgoingMessage::text(response_text))
                            .await;
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to process WhatsApp Business message");
                    let _ = self
                        .adapter
                        .send_message(
                            &msg.from,
                            OutgoingMessage::text(
                                "Sorry, I encountered an error. Please try again.",
                            ),
                        )
                        .await;
                }
            }
        }

        Ok(())
    }
}
