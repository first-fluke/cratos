//! Telegram message handler and bot runner

use super::adapter::TelegramAdapter;
use super::commands::handle_slash_command;
use crate::error::Result;
use crate::message::{ChannelAdapter, OutgoingAttachment};
use crate::util::{markdown_to_html, mask_for_logging, sanitize_error_for_user};
use cratos_core::dev_sessions::DevSessionMonitor;
use cratos_core::{Orchestrator, OrchestratorInput};
use cratos_llm::ImageContent;
use std::sync::Arc;
use teloxide::{
    net::Download,
    payloads::SendMessageSetters,
    prelude::*,
    types::{
        ChatAction, ChatId, FileId, InlineKeyboardButton, InlineKeyboardMarkup,
        Message as TelegramMessage, ParseMode, ReplyParameters,
    },
};
use tracing::{debug, error, info, instrument, warn};

impl TelegramAdapter {
    /// Start the bot with the given orchestrator and optional dev session monitor
    #[instrument(skip(self, orchestrator, dev_monitor))]
    pub async fn run(
        self: Arc<Self>,
        orchestrator: Arc<Orchestrator>,
        dev_monitor: Option<Arc<DevSessionMonitor>>,
    ) -> Result<()> {
        info!("Starting Telegram bot");

        let bot = self.bot.clone();
        let adapter = self.clone();

        // Spawn EventBus notification listener if notify_chat_id is set
        let _notify_handle = if let (Some(notify_chat_id), Some(bus)) = (
            self.config.notify_chat_id,
            orchestrator.event_bus().cloned(),
        ) {
            let notify_bot = self.bot.clone();
            let chat_id = ChatId(notify_chat_id);
            let mut rx = bus.subscribe();
            Some(tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(cratos_core::event_bus::OrchestratorEvent::ApprovalRequired {
                            execution_id,
                            request_id,
                        }) => {
                            let text = format!(
                                "Approval required for execution <code>{}</code>\n\
                                 Request ID: <code>{}</code>\n\
                                 Use /approve {} to approve.",
                                execution_id, request_id, request_id
                            );
                            let buttons = vec![
                                InlineKeyboardButton::callback(
                                    "Approve",
                                    format!("approve:{}", request_id),
                                ),
                                InlineKeyboardButton::callback(
                                    "Deny",
                                    format!("deny:{}", request_id),
                                ),
                            ];
                            let keyboard = InlineKeyboardMarkup::new(vec![buttons]);
                            let _ = notify_bot
                                .send_message(chat_id, &text)
                                .parse_mode(ParseMode::Html)
                                .reply_markup(keyboard)
                                .await;
                        }
                        Ok(cratos_core::event_bus::OrchestratorEvent::ExecutionFailed {
                            execution_id,
                            error,
                        }) => {
                            let text = format!(
                                "Execution <code>{}</code> failed:\n{}",
                                execution_id,
                                sanitize_error_for_user(&error)
                            );
                            let _ = notify_bot
                                .send_message(chat_id, &text)
                                .parse_mode(ParseMode::Html)
                                .await;
                        }
                        Ok(cratos_core::event_bus::OrchestratorEvent::QuotaWarning {
                            provider,
                            remaining_pct,
                            reset_in_secs,
                        }) => {
                            let reset_info = reset_in_secs
                                .filter(|&s| s > 0)
                                .map(|s| {
                                    let mins = s / 60;
                                    if mins > 0 {
                                        format!(", resets in {}m", mins)
                                    } else {
                                        format!(", resets in {}s", s)
                                    }
                                })
                                .unwrap_or_default();

                            let emoji = if remaining_pct < 10.0 {
                                "🔴"
                            } else {
                                "⚠️"
                            };
                            let text = format!(
                                "{} <b>Quota Warning</b>\n\
                                Provider: <code>{}</code>\n\
                                Remaining: {:.1}%{}",
                                emoji, provider, remaining_pct, reset_info
                            );

                            if let Err(e) = notify_bot
                                .send_message(chat_id, &text)
                                .parse_mode(ParseMode::Html)
                                .await
                            {
                                warn!(error = %e, "Failed to send quota warning");
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            debug!(skipped = n, "EventBus notification listener lagged");
                        }
                        _ => {}
                    }
                }
            }))
        } else {
            None
        };

        let handler = Update::filter_message().endpoint(move |bot: Bot, msg: TelegramMessage| {
            let adapter = adapter.clone();
            let orchestrator = orchestrator.clone();
            let dev_monitor = dev_monitor.clone();
            async move { Self::handle_message(adapter, orchestrator, dev_monitor, bot, msg).await }
        });

        // Use unique distribution key per message to allow concurrent handling.
        // This ensures /cancel can be processed while another command is running.
        Dispatcher::builder(bot, handler)
            .distribution_function(|_| None::<std::convert::Infallible>)
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;

        Ok(())
    }

    /// Handle an incoming message
    pub(crate) async fn handle_message(
        adapter: Arc<Self>,
        orchestrator: Arc<Orchestrator>,
        dev_monitor: Option<Arc<DevSessionMonitor>>,
        bot: Bot,
        msg: TelegramMessage,
    ) -> ResponseResult<()> {
        let bot_username = bot
            .get_me()
            .await
            .map(|me| me.username.clone().unwrap_or_default())
            .unwrap_or_default();

        let Some(normalized) = adapter.normalize_message(&msg, &bot_username) else {
            return Ok(());
        };

        // Check for slash commands before orchestrator processing
        let text = normalized.text.trim();
        if text.starts_with('/') {
            let mut parts = text.splitn(2, ' ');
            let command = parts.next().unwrap_or("");
            // Strip @bot_username suffix from commands (e.g. /status@mybot)
            let command = command.split('@').next().unwrap_or(command);
            let args = parts.next().unwrap_or("").trim();

            if let Some(result) = handle_slash_command(
                command,
                args,
                &orchestrator,
                &dev_monitor,
                &bot,
                msg.chat.id,
                msg.id,
            )
            .await
            {
                return result;
            }
        }

        // SECURITY: Mask potentially sensitive content in logs
        info!(
            chat_id = %normalized.channel_id,
            user_id = %normalized.user_id,
            text = %mask_for_logging(&normalized.text),
            "Received message"
        );

        // Send typing indicator
        let _ = bot.send_chat_action(msg.chat.id, ChatAction::Typing).await;

        // Send "processing..." placeholder for progressive updates
        let progress_msg = bot
            .send_message(msg.chat.id, "처리 중...")
            .reply_parameters(ReplyParameters::new(msg.id))
            .await;

        // Spawn progressive update task if EventBus is available
        let progress_handle = if let (Ok(ref pm), Some(bus)) =
            (&progress_msg, orchestrator.event_bus().cloned())
        {
            let bot_clone = bot.clone();
            let chat_id = msg.chat.id;
            let progress_msg_id = pm.id;
            let mut rx = bus.subscribe();
            Some(tokio::spawn(async move {
                let mut last_edit = std::time::Instant::now();
                let min_interval = std::time::Duration::from_secs(2);
                let mut tool_count = 0u32;
                while let Ok(event) = rx.recv().await {
                    match event {
                        cratos_core::event_bus::OrchestratorEvent::ToolStarted {
                            tool_name,
                            ..
                        } => {
                            tool_count += 1;
                            let now = std::time::Instant::now();
                            if now.duration_since(last_edit) >= min_interval {
                                let text = format!(
                                    "처리 중... [{}] 실행 중 ({}번째 도구)",
                                    tool_name, tool_count
                                );
                                let _ = bot_clone
                                    .edit_message_text(chat_id, progress_msg_id, &text)
                                    .await;
                                last_edit = now;
                            }
                        }
                        cratos_core::event_bus::OrchestratorEvent::ExecutionCompleted {
                            ..
                        }
                        | cratos_core::event_bus::OrchestratorEvent::ExecutionFailed { .. } => {
                            break;
                        }
                        _ => {}
                    }
                }
            }))
        } else {
            None
        };

        // Download images from attachments (multimodal support)
        let mut images = Vec::new();
        for att in &normalized.attachments {
            if att.attachment_type == crate::message::AttachmentType::Image {
                if let Some(file_id) = &att.file_id {
                    match bot.get_file(FileId(file_id.clone())).await {
                        Ok(file) => {
                            let mut buf = Vec::new();
                            match bot.download_file(&file.path, &mut buf).await {
                                Ok(()) => {
                                    let mime = att
                                        .mime_type
                                        .clone()
                                        .unwrap_or_else(|| "image/jpeg".to_string());
                                    images.push(ImageContent::new(mime, buf));
                                    tracing::info!(file_id = %file_id, "Downloaded Telegram photo for multimodal");
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, file_id = %file_id, "Failed to download photo");
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, file_id = %file_id, "Failed to get file info");
                        }
                    }
                }
            }
        }

        // If photo-only (no caption), use a default prompt
        let text = if normalized.text.is_empty() && !images.is_empty() {
            "이 이미지를 분석해주세요.".to_string()
        } else {
            normalized.text.clone()
        };

        // Process with orchestrator
        let mut input = OrchestratorInput::new(
            "telegram",
            &normalized.channel_id,
            &normalized.user_id,
            &text,
        );
        if !images.is_empty() {
            input = input.with_images(images);
        }

        // Record channel message metric
        cratos_core::metrics_global::labeled_counter("cratos_channel_messages_total")
            .inc(&[("channel_type", "telegram")]);

        match orchestrator.process(input).await {
            Ok(result) => {
                // Cancel progress listener and wait for it to fully stop
                // to avoid race conditions where a late progress edit
                // overwrites the final response
                if let Some(h) = progress_handle {
                    h.abort();
                    let _ = h.await; // wait for abort to complete
                }

                let response_text = if result.response.is_empty() {
                    "I've completed the task.".to_string()
                } else {
                    result.response
                };

                // Convert standard Markdown (LLM output) to Telegram-safe HTML
                let html_text = markdown_to_html(&response_text);

                // Delete the progress message first to avoid race conditions,
                // then send the final response as a new message.
                // Previously we tried edit_message_text but a late progress
                // edit could overwrite the final response.
                if let Ok(ref pm) = progress_msg {
                    let _ = bot.delete_message(msg.chat.id, pm.id).await;
                }

                let send_result = bot
                    .send_message(msg.chat.id, &html_text)
                    .parse_mode(ParseMode::Html)
                    .reply_parameters(ReplyParameters::new(msg.id))
                    .await;

                // Fall back to plain text if HTML parsing fails
                match &send_result {
                    Ok(sent_msg) => {
                        info!(
                            chat_id = %msg.chat.id,
                            message_id = %sent_msg.id,
                            response_len = response_text.len(),
                            "Sent response message (HTML)"
                        );
                    }
                    Err(e) => {
                        warn!(
                            chat_id = %msg.chat.id,
                            error = %e,
                            "HTML send failed, falling back to plain text"
                        );
                        match bot
                            .send_message(msg.chat.id, &response_text)
                            .reply_parameters(ReplyParameters::new(msg.id))
                            .await
                        {
                            Ok(sent_msg) => {
                                info!(
                                    chat_id = %msg.chat.id,
                                    message_id = %sent_msg.id,
                                    response_len = response_text.len(),
                                    "Sent response message (plain text fallback)"
                                );
                            }
                            Err(e2) => {
                                error!(
                                    chat_id = %msg.chat.id,
                                    error = %e2,
                                    "Failed to send response message"
                                );
                            }
                        }
                    }
                }

                // Handle artifacts (e.g. screenshots, files)
                for artifact in &result.artifacts {
                    let attachment = OutgoingAttachment {
                        filename: artifact.name.clone(),
                        mime_type: artifact.mime_type.clone(),
                        data: artifact.data.clone(),
                        attachment_type: if artifact.mime_type.starts_with("image/") {
                            crate::message::AttachmentType::Image
                        } else if artifact.mime_type.starts_with("audio/") {
                            crate::message::AttachmentType::Audio
                        } else if artifact.mime_type.starts_with("video/") {
                            crate::message::AttachmentType::Video
                        } else {
                            crate::message::AttachmentType::Document
                        },
                        caption: Some(format!("Artifact: {}", artifact.name)),
                    };

                    let channel_id = msg.chat.id.0.to_string();
                    let reply_to = Some(msg.id.0.to_string());
                    if let Err(e) = adapter
                        .send_attachment(&channel_id, attachment, reply_to.as_deref())
                        .await
                    {
                        error!(error = %e, artifact = %artifact.name, "Failed to send artifact");
                    }
                }
            }
            Err(e) => {
                // Cancel progress listener
                if let Some(h) = progress_handle {
                    h.abort();
                }

                // Delete progress message
                if let Ok(ref pm) = progress_msg {
                    let _ = bot.delete_message(msg.chat.id, pm.id).await;
                }

                // Log full error internally
                error!(error = %e, "Failed to process message");

                // SECURITY: Send sanitized error to user (don't expose internal details)
                let user_message = sanitize_error_for_user(&e.to_string());
                let _ = bot
                    .send_message(
                        msg.chat.id,
                        format!("Sorry, I encountered an error: {}", user_message),
                    )
                    .reply_parameters(ReplyParameters::new(msg.id))
                    .await;
            }
        }

        Ok(())
    }
}
