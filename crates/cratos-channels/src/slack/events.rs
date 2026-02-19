use super::SlackAdapter;
use crate::error::{Error, Result};
use crate::message::{
    AttachmentType, ChannelAdapter, ChannelType, NormalizedMessage, OutgoingAttachment,
    OutgoingMessage,
};
use cratos_core::{Orchestrator, OrchestratorInput};
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Shared state passed to Socket Mode callbacks via user state.
pub(crate) struct SocketModeState {
    pub(crate) adapter: Arc<SlackAdapter>,
    pub(crate) orchestrator: Arc<Orchestrator>,
}

/// Socket Mode push event handler (plain function, no captures).
pub(crate) async fn socket_mode_push_handler(
    event: SlackPushEventCallback,
    _client: Arc<SlackHyperClient>,
    states: SlackClientEventsUserState,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state_guard = states.read().await;
    let Some(state) = state_guard.get_user_state::<SocketModeState>() else {
        warn!("SocketModeState not found in user state");
        return Ok(());
    };

    let adapter = &state.adapter;
    let orchestrator = &state.orchestrator;

    // Workspace allow-list check
    if !adapter.is_workspace_allowed(event.team_id.as_ref()) {
        debug!(team_id = %event.team_id, "Workspace not allowed, ignoring");
        return Ok(());
    }

    match event.event {
        SlackEventCallbackBody::Message(msg) => {
            handle_message_event(adapter, orchestrator, msg).await;
        }
        SlackEventCallbackBody::AppMention(mention) => {
            handle_app_mention_event(adapter, orchestrator, mention).await;
        }
        _ => {
            debug!("Unhandled Slack event type, ignoring");
        }
    }

    Ok(())
}

/// Handle a Slack message event from Socket Mode.
pub(crate) async fn handle_message_event(
    adapter: &SlackAdapter,
    orchestrator: &Orchestrator,
    msg: SlackMessageEvent,
) {
    // Skip bot messages (including our own) to avoid loops
    if msg.sender.bot_id.is_some() {
        return;
    }

    let Some(channel_id) = msg.origin.channel.as_ref() else {
        return;
    };
    let Some(user_id) = msg.sender.user.as_ref() else {
        return;
    };
    let text = msg
        .content
        .as_ref()
        .and_then(|c| c.text.as_ref())
        .cloned()
        .unwrap_or_default();

    let ts = msg.origin.ts.to_string();
    let thread_ts = msg.origin.thread_ts.as_ref().map(|t| t.to_string());

    if let Err(e) = adapter
        .process_message(
            orchestrator,
            channel_id.as_ref(),
            user_id.as_ref(),
            &text,
            &ts,
            thread_ts.as_deref(),
        )
        .await
    {
        error!(error = %e, "Failed to process Slack message event");
    }
}

/// Handle a Slack app_mention event from Socket Mode.
pub(crate) async fn handle_app_mention_event(
    adapter: &SlackAdapter,
    orchestrator: &Orchestrator,
    mention: SlackAppMentionEvent,
) {
    let channel_id = mention.channel.to_string();
    let user_id = mention.user.to_string();
    let text = mention.content.text.as_ref().cloned().unwrap_or_default();
    let ts = mention.origin.ts.to_string();
    let thread_ts = mention.origin.thread_ts.as_ref().map(|t| t.to_string());

    if let Err(e) = adapter
        .process_message(
            orchestrator,
            &channel_id,
            &user_id,
            &text,
            &ts,
            thread_ts.as_deref(),
        )
        .await
    {
        error!(error = %e, "Failed to process Slack app_mention event");
    }
}

/// Socket Mode interaction event handler (button clicks, etc.).
pub(crate) async fn socket_mode_interaction_handler(
    event: SlackInteractionEvent,
    _client: Arc<SlackHyperClient>,
    states: SlackClientEventsUserState,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state_guard = states.read().await;
    let Some(state) = state_guard.get_user_state::<SocketModeState>() else {
        warn!("SocketModeState not found in user state");
        return Ok(());
    };

    match event {
        SlackInteractionEvent::BlockActions(action_event) => {
            handle_block_actions(state, action_event).await;
        }
        _ => {
            debug!("Unhandled Slack interaction type, ignoring");
        }
    }

    Ok(())
}

/// Handle block action interactions (button clicks).
pub(crate) async fn handle_block_actions(
    state: &SocketModeState,
    event: SlackInteractionBlockActionsEvent,
) {
    let actions = match &event.actions {
        Some(actions) if !actions.is_empty() => actions,
        _ => return,
    };

    let user_id = event
        .user
        .as_ref()
        .map(|u| u.id.to_string())
        .unwrap_or_default();
    let channel_id = event
        .channel
        .as_ref()
        .map(|c| c.id.to_string())
        .unwrap_or_default();

    for action in actions {
        let action_id = action.action_id.to_string();
        debug!(
            action_id = %action_id,
            user = %user_id,
            channel = %channel_id,
            "Processing block action"
        );

        // Route the action as a message to the orchestrator
        let text = format!("/action {}", action_id);
        let ts = action
            .action_ts
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_default();

        if let Err(e) = state
            .adapter
            .process_message(&state.orchestrator, &channel_id, &user_id, &text, &ts, None)
            .await
        {
            error!(error = %e, action_id = %action_id, "Failed to process block action");
        }
    }
}

impl SlackAdapter {
    /// Convert a Slack message event to a normalized message
    pub async fn normalize_message(
        &self,
        channel_id: &str,
        user_id: &str,
        text: &str,
        ts: &str,
        thread_ts: Option<&str>,
    ) -> Option<NormalizedMessage> {
        // Skip empty messages
        if text.is_empty() {
            return None;
        }

        // Check channel permissions
        if !self.is_channel_allowed(channel_id) {
            debug!(channel_id = %channel_id, "Channel not allowed");
            return None;
        }

        // Check mentions if required
        if self.config.mentions_only {
            let is_dm = channel_id.starts_with('D'); // DM channels start with D
            let is_mentioned = self.is_bot_mentioned(text).await;

            if !is_dm && !is_mentioned {
                return None;
            }
        }

        let mut normalized = NormalizedMessage::new(
            ChannelType::Slack,
            channel_id.to_string(),
            user_id.to_string(),
            ts.to_string(),
            text.to_string(),
        );

        // Handle thread context
        if let Some(thread) = thread_ts {
            normalized = normalized.with_thread(thread.to_string());
            normalized.is_reply = true;
        }

        Some(normalized)
    }

    /// Process an incoming message (called from webhook or socket mode)
    pub async fn process_message(
        &self,
        orchestrator: &Orchestrator,
        channel: &str,
        user: &str,
        text: &str,
        ts: &str,
        thread_ts: Option<&str>,
    ) -> Result<Option<String>> {
        // Normalize the message
        let Some(normalized) = self
            .normalize_message(channel, user, text, ts, thread_ts)
            .await
        else {
            return Ok(None);
        };

        info!(
            channel_id = %normalized.channel_id,
            user_id = %normalized.user_id,
            text = %normalized.text,
            "Processing Slack message"
        );

        // Process with orchestrator
        let input = OrchestratorInput::new(
            "slack",
            &normalized.channel_id,
            &normalized.user_id,
            &normalized.text,
        );

        match orchestrator.process(input).await {
            Ok(result) => {
                let response_text = if result.response.is_empty() {
                    "I've completed the task.".to_string()
                } else {
                    result.response
                };

                // Send response
                let reply_thread = thread_ts.unwrap_or(ts);
                let message =
                    OutgoingMessage::text(response_text).in_thread(reply_thread.to_string());

                let _ = self.send_message(channel, message).await?;

                // Handle artifacts (files, images)
                for artifact in &result.artifacts {
                    let attachment = OutgoingAttachment {
                        filename: artifact.name.clone(),
                        mime_type: artifact.mime_type.clone(),
                        data: artifact.data.clone(),
                        attachment_type: if artifact.mime_type.starts_with("image/") {
                            AttachmentType::Image
                        } else if artifact.mime_type.starts_with("audio/") {
                            AttachmentType::Audio
                        } else if artifact.mime_type.starts_with("video/") {
                            AttachmentType::Video
                        } else {
                            AttachmentType::Document
                        },
                        caption: Some(format!("Artifact: {}", artifact.name)),
                    };

                    if let Err(e) = self
                        .send_attachment(channel, attachment, Some(reply_thread))
                        .await
                    {
                        error!(error = %e, artifact = %artifact.name, "Failed to send Slack artifact");
                    }
                }

                Ok(Some("Message sent".to_string()))
            }
            Err(e) => {
                error!(error = %e, "Failed to process Slack message");

                let error_message =
                    OutgoingMessage::text(format!("Sorry, I encountered an error: {}", e))
                        .in_thread(ts.to_string());
                let _ = self.send_message(channel, error_message).await;

                Err(Error::Slack(format!("Processing error: {}", e)))
            }
        }
    }
}
