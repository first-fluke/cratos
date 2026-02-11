//! Telegram - teloxide adapter
//!
//! This module provides the Telegram bot adapter using the teloxide library.

use crate::error::{Error, Result};
use crate::message::{
    Attachment, AttachmentType, ChannelAdapter, ChannelType, MessageButton, NormalizedMessage,
    OutgoingMessage,
};
use crate::util::{markdown_to_html, mask_for_logging, sanitize_error_for_user};
use cratos_core::dev_sessions::DevSessionMonitor;
use cratos_core::{Orchestrator, OrchestratorInput};
use std::sync::Arc;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    types::{
        ChatAction, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, Message as TelegramMessage,
        MessageId, ParseMode, ReplyParameters,
    },
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use tracing::{debug, error, info, instrument};

/// DM security policy for Telegram
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmPolicy {
    /// Require pairing code before accepting DMs from unknown users
    Pairing,
    /// Only accept DMs from users in the allowed_users list
    Allowlist,
    /// Accept DMs from any user (least secure)
    Open,
    /// Disable DM handling entirely
    Disabled,
}

impl Default for DmPolicy {
    fn default() -> Self {
        Self::Allowlist
    }
}

/// Telegram bot configuration
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    /// Bot token
    pub bot_token: String,
    /// Allowed user IDs (empty = allow all)
    pub allowed_users: Vec<i64>,
    /// Allowed group IDs (empty = allow all)
    pub allowed_groups: Vec<i64>,
    /// Whether to respond only to mentions/replies in groups
    pub groups_mention_only: bool,
    /// DM security policy
    pub dm_policy: DmPolicy,
    /// Chat ID for system notifications (approval requests, errors, etc.)
    pub notify_chat_id: Option<i64>,
}

impl TelegramConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| Error::Telegram("TELEGRAM_BOT_TOKEN not set".to_string()))?;

        let allowed_users: Vec<i64> = std::env::var("TELEGRAM_ALLOWED_USERS")
            .ok()
            .map(|s| {
                s.split(',')
                    .filter_map(|id| id.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default();

        let allowed_groups: Vec<i64> = std::env::var("TELEGRAM_ALLOWED_GROUPS")
            .ok()
            .map(|s| {
                s.split(',')
                    .filter_map(|id| id.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default();

        let groups_mention_only = std::env::var("TELEGRAM_GROUPS_MENTION_ONLY")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(true);

        let dm_policy = std::env::var("TELEGRAM_DM_POLICY")
            .ok()
            .map(|s| match s.to_lowercase().as_str() {
                "pairing" => DmPolicy::Pairing,
                "allowlist" => DmPolicy::Allowlist,
                "open" => DmPolicy::Open,
                "disabled" => DmPolicy::Disabled,
                _ => DmPolicy::default(),
            })
            .unwrap_or_default();

        let notify_chat_id = std::env::var("TELEGRAM_NOTIFY_CHAT_ID")
            .ok()
            .and_then(|s| s.trim().parse().ok());

        Ok(Self {
            bot_token,
            allowed_users,
            allowed_groups,
            groups_mention_only,
            dm_policy,
            notify_chat_id,
        })
    }

    /// Create with a bot token
    #[must_use]
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            allowed_users: Vec::new(),
            allowed_groups: Vec::new(),
            groups_mention_only: true,
            dm_policy: DmPolicy::default(),
            notify_chat_id: None,
        }
    }

    /// Set allowed users
    #[must_use]
    pub fn with_allowed_users(mut self, users: Vec<i64>) -> Self {
        self.allowed_users = users;
        self
    }

    /// Set allowed groups
    #[must_use]
    pub fn with_allowed_groups(mut self, groups: Vec<i64>) -> Self {
        self.allowed_groups = groups;
        self
    }

    /// Set groups mention only mode
    #[must_use]
    pub fn with_groups_mention_only(mut self, enabled: bool) -> Self {
        self.groups_mention_only = enabled;
        self
    }

    /// Set DM security policy
    #[must_use]
    pub fn with_dm_policy(mut self, policy: DmPolicy) -> Self {
        self.dm_policy = policy;
        self
    }

    /// Set chat ID for system notifications
    #[must_use]
    pub fn with_notify_chat_id(mut self, chat_id: i64) -> Self {
        self.notify_chat_id = Some(chat_id);
        self
    }
}

/// Telegram bot adapter
pub struct TelegramAdapter {
    bot: Bot,
    config: TelegramConfig,
}

impl TelegramAdapter {
    /// Create a new Telegram adapter
    #[must_use]
    pub fn new(config: TelegramConfig) -> Self {
        let bot = Bot::new(&config.bot_token);
        Self { bot, config }
    }

    /// Create from environment
    pub fn from_env() -> Result<Self> {
        let config = TelegramConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Get the underlying bot
    pub fn bot(&self) -> &Bot {
        &self.bot
    }

    /// Check if a user is allowed
    pub fn is_user_allowed(&self, user_id: i64) -> bool {
        self.config.allowed_users.is_empty() || self.config.allowed_users.contains(&user_id)
    }

    /// Check if a group is allowed
    pub fn is_group_allowed(&self, chat_id: i64) -> bool {
        self.config.allowed_groups.is_empty() || self.config.allowed_groups.contains(&chat_id)
    }

    /// Convert a Telegram message to a normalized message
    pub fn normalize_message(
        &self,
        msg: &TelegramMessage,
        bot_username: &str,
    ) -> Option<NormalizedMessage> {
        let text = msg.text().unwrap_or("").to_string();

        // Skip empty messages
        if text.is_empty() {
            return None;
        }

        let user = msg.from.as_ref()?;
        let chat_id = msg.chat.id.0;
        let user_id = user.id.0;

        // Check permissions
        let is_dm = msg.chat.is_private();

        if is_dm {
            match self.config.dm_policy {
                DmPolicy::Disabled => {
                    debug!(user_id = %user_id, "DMs are disabled");
                    return None;
                }
                DmPolicy::Allowlist => {
                    if !self.is_user_allowed(user_id as i64) {
                        debug!(user_id = %user_id, "User not in allowlist");
                        return None;
                    }
                }
                DmPolicy::Pairing => {
                    // Pairing mode: allow listed users, block unknown
                    // Full pairing code flow would require state storage
                    if !self.is_user_allowed(user_id as i64) {
                        debug!(user_id = %user_id, "User not paired");
                        return None;
                    }
                }
                DmPolicy::Open => {
                    // Accept all DMs
                }
            }
        } else if !self.is_user_allowed(user_id as i64) {
            debug!(user_id = %user_id, "User not allowed");
            return None;
        }

        if msg.chat.is_group() || msg.chat.is_supergroup() {
            if !self.is_group_allowed(chat_id) {
                debug!(chat_id = %chat_id, "Group not allowed");
                return None;
            }

            // In groups, check if we should respond
            if self.config.groups_mention_only {
                let is_reply_to_bot = msg
                    .reply_to_message()
                    .and_then(|r| r.from.as_ref())
                    .map(|u| u.username.as_deref() == Some(bot_username))
                    .unwrap_or(false);

                let is_mention = text.contains(&format!("@{}", bot_username));

                if !is_reply_to_bot && !is_mention {
                    return None;
                }
            }
        }

        let user_name = user.first_name.clone();
        let message_id = msg.id.0.to_string();

        let mut normalized = NormalizedMessage::new(
            ChannelType::Telegram,
            chat_id.to_string(),
            user_id.to_string(),
            message_id,
            text,
        )
        .with_user_name(user_name);

        // Handle reply context
        if let Some(reply) = msg.reply_to_message() {
            normalized = normalized.with_thread(reply.id.0.to_string());
            normalized.is_reply = true;
        }

        // Handle attachments
        if let Some(photo) = msg.photo() {
            if let Some(largest) = photo.last() {
                normalized = normalized.with_attachment(Attachment {
                    attachment_type: AttachmentType::Image,
                    file_name: None,
                    mime_type: Some("image/jpeg".to_string()),
                    file_size: Some(largest.file.size as u64),
                    url: None,
                    file_id: Some(largest.file.id.to_string()),
                });
            }
        }

        if let Some(doc) = msg.document() {
            normalized = normalized.with_attachment(Attachment {
                attachment_type: AttachmentType::Document,
                file_name: doc.file_name.clone(),
                mime_type: doc.mime_type.as_ref().map(|m| m.to_string()),
                file_size: Some(doc.file.size as u64),
                url: None,
                file_id: Some(doc.file.id.to_string()),
            });
        }

        Some(normalized)
    }

    /// Build inline keyboard from buttons
    fn build_keyboard(buttons: &[MessageButton]) -> Option<InlineKeyboardMarkup> {
        if buttons.is_empty() {
            return None;
        }

        let keyboard_buttons: Vec<InlineKeyboardButton> = buttons
            .iter()
            .filter_map(|b| {
                if let Some(callback_data) = &b.callback_data {
                    Some(InlineKeyboardButton::callback(&b.text, callback_data))
                } else if let Some(url) = &b.url {
                    Some(InlineKeyboardButton::url(&b.text, url.parse().ok()?))
                } else {
                    None
                }
            })
            .collect();

        // Single row for simplicity
        Some(InlineKeyboardMarkup::new(vec![keyboard_buttons]))
    }

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
        let _notify_handle = if let (Some(notify_chat_id), Some(bus)) =
            (self.config.notify_chat_id, orchestrator.event_bus().cloned())
        {
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

        Dispatcher::builder(bot, handler)
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;

        Ok(())
    }

    /// Handle a slash command (e.g. /status, /sessions, /tools, /cancel, /approve)
    async fn handle_slash_command(
        command: &str,
        args: &str,
        orchestrator: &Arc<Orchestrator>,
        dev_monitor: &Option<Arc<DevSessionMonitor>>,
        bot: &Bot,
        chat_id: ChatId,
        reply_to: MessageId,
    ) -> Option<ResponseResult<()>> {
        let response = match command {
            "/status" => {
                let mut lines = vec!["<b>System Status</b>".to_string()];

                // Active dev sessions
                if let Some(monitor) = dev_monitor {
                    let sessions = monitor.sessions().await;
                    if sessions.is_empty() {
                        lines.push("AI Sessions: None active".to_string());
                    } else {
                        lines.push(format!("AI Sessions: {} active", sessions.len()));
                        for s in &sessions {
                            lines.push(format!(
                                "  - {:?} ({:?}) @ {}",
                                s.tool, s.status, s.project_path.as_deref().unwrap_or("unknown")
                            ));
                        }
                    }
                } else {
                    lines.push("AI Sessions: Monitor not available".to_string());
                }

                // Active executions
                if let Some(count) = orchestrator.active_execution_count() {
                    lines.push(format!("Active executions: {}", count));
                }

                lines.join("\n")
            }
            "/sessions" => {
                if let Some(monitor) = dev_monitor {
                    let sessions = monitor.sessions().await;
                    if sessions.is_empty() {
                        "No active AI development sessions.".to_string()
                    } else {
                        let mut lines = vec![format!("<b>Active AI Sessions ({})</b>", sessions.len())];
                        for (i, s) in sessions.iter().enumerate() {
                            lines.push(format!(
                                "{}. <b>{:?}</b> - {:?}\n   Path: <code>{}</code>\n   PID: {:?}",
                                i + 1,
                                s.tool,
                                s.status,
                                s.project_path.as_deref().unwrap_or("unknown"),
                                s.pid,
                            ));
                        }
                        lines.join("\n")
                    }
                } else {
                    "DevSessionMonitor not available.".to_string()
                }
            }
            "/tools" => {
                let tool_names = orchestrator.list_tool_names();
                if tool_names.is_empty() {
                    "No tools registered.".to_string()
                } else {
                    let mut lines = vec![format!("<b>Available Tools ({})</b>", tool_names.len())];
                    for name in &tool_names {
                        lines.push(format!("  - <code>{}</code>", name));
                    }
                    lines.join("\n")
                }
            }
            "/cancel" => {
                if args.is_empty() {
                    "Usage: /cancel &lt;execution_id&gt;".to_string()
                } else if let Ok(exec_id) = args.parse::<uuid::Uuid>() {
                    if orchestrator.cancel_execution(exec_id) {
                        format!("Cancelled execution <code>{}</code>", exec_id)
                    } else {
                        format!("Execution <code>{}</code> not found or already completed.", exec_id)
                    }
                } else {
                    "Invalid execution ID. Please provide a valid UUID.".to_string()
                }
            }
            "/approve" => {
                if args.is_empty() {
                    "Usage: /approve &lt;request_id&gt;".to_string()
                } else {
                    // Delegate to orchestrator, which may not have approval manager here
                    format!("Approval for <code>{}</code> — use WebSocket gateway for full approval flow.", args)
                }
            }
            "/agent" => {
                if args.is_empty() {
                    "Usage: /agent &lt;claude|codex|gemini|antigravity&gt; &lt;prompt&gt;\n\
                     Example: /agent claude Fix the bug in auth.rs"
                        .to_string()
                } else {
                    // Parse: first word = agent name, rest = prompt
                    let mut agent_parts = args.splitn(2, ' ');
                    let agent_name = agent_parts.next().unwrap_or("");
                    let agent_prompt = agent_parts.next().unwrap_or("").trim();

                    if agent_prompt.is_empty() {
                        format!(
                            "Usage: /agent {} &lt;prompt&gt;\nPlease provide a task description.",
                            agent_name
                        )
                    } else {
                        // Delegate to orchestrator as a tool call request
                        let request = format!(
                            "agent_cli 도구를 사용해서 {}에게 다음 작업을 시켜줘: {}",
                            agent_name, agent_prompt
                        );

                        // Return None to let the orchestrator handle this
                        // by falling through to the normal message processing
                        // We rewrite the text so the orchestrator picks it up
                        let chat_id_str = chat_id.0.to_string();
                        let input = OrchestratorInput::new(
                            "telegram",
                            &chat_id_str,
                            &chat_id_str,
                            &request,
                        );

                        match orchestrator.process(input).await {
                            Ok(result) => {
                                let text = if result.response.is_empty() {
                                    format!("Agent '{}' task completed.", agent_name)
                                } else {
                                    result.response
                                };
                                crate::util::markdown_to_html(&text)
                            }
                            Err(e) => {
                                format!("Agent error: {}", crate::util::sanitize_error_for_user(&e.to_string()))
                            }
                        }
                    }
                }
            }
            _ => return None,
        };

        let result = bot
            .send_message(chat_id, &response)
            .parse_mode(ParseMode::Html)
            .reply_parameters(ReplyParameters::new(reply_to))
            .await;

        if result.is_err() {
            // Fallback to plain text
            let _ = bot
                .send_message(chat_id, &response)
                .reply_parameters(ReplyParameters::new(reply_to))
                .await;
        }

        Some(Ok(()))
    }

    /// Handle an incoming message
    async fn handle_message(
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

            if let Some(result) = Self::handle_slash_command(
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
                            tool_name, ..
                        } => {
                            tool_count += 1;
                            let now = std::time::Instant::now();
                            if now.duration_since(last_edit) >= min_interval {
                                let text = format!("처리 중... [{}] 실행 중 ({}번째 도구)", tool_name, tool_count);
                                let _ = bot_clone
                                    .edit_message_text(chat_id, progress_msg_id, &text)
                                    .await;
                                last_edit = now;
                            }
                        }
                        cratos_core::event_bus::OrchestratorEvent::ExecutionCompleted { .. }
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

        // Process with orchestrator
        let input = OrchestratorInput::new(
            "telegram",
            &normalized.channel_id,
            &normalized.user_id,
            &normalized.text,
        );

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
                if send_result.is_err() {
                    let _ = bot
                        .send_message(msg.chat.id, &response_text)
                        .reply_parameters(ReplyParameters::new(msg.id))
                        .await;
                }

                // Handle artifacts (e.g. screenshots)
                for artifact in &result.artifacts {
                    if artifact.mime_type.starts_with("image/") {
                        if let Ok(data) = BASE64.decode(&artifact.data) {
                            let _ = bot
                                .send_photo(msg.chat.id, InputFile::memory(data))
                                .caption(format!("Artifact: {}", artifact.name))
                                .reply_parameters(ReplyParameters::new(msg.id))
                                .await
                                .map_err(|e| error!(error = %e, artifact = %artifact.name, "Failed to send photo artifact"));
                        } else {
                            error!(artifact = %artifact.name, "Failed to decode base64 data for artifact");
                        }
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

#[async_trait::async_trait]
impl ChannelAdapter for TelegramAdapter {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String> {
        let chat_id: i64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid chat ID".to_string()))?;

        let mut request = self.bot.send_message(ChatId(chat_id), &message.text);

        if message.parse_markdown {
            let html_text = markdown_to_html(&message.text);
            request = self.bot.send_message(ChatId(chat_id), &html_text)
                .parse_mode(ParseMode::Html);
        }

        if let Some(reply_to) = &message.reply_to {
            if let Ok(msg_id) = reply_to.parse::<i32>() {
                request = request.reply_parameters(ReplyParameters::new(MessageId(msg_id)));
            }
        }

        if let Some(keyboard) = Self::build_keyboard(&message.buttons) {
            request = request.reply_markup(keyboard);
        }

        let sent = request.await.map_err(|e| Error::Telegram(e.to_string()))?;

        Ok(sent.id.0.to_string())
    }

    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        message: OutgoingMessage,
    ) -> Result<()> {
        let chat_id: i64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid chat ID".to_string()))?;
        let msg_id: i32 = message_id
            .parse()
            .map_err(|_| Error::Parse("Invalid message ID".to_string()))?;

        let mut request =
            self.bot
                .edit_message_text(ChatId(chat_id), MessageId(msg_id), &message.text);

        if message.parse_markdown {
            let html_text = markdown_to_html(&message.text);
            request = self.bot.edit_message_text(ChatId(chat_id), MessageId(msg_id), &html_text)
                .parse_mode(ParseMode::Html);
        }

        request.await.map_err(|e| Error::Telegram(e.to_string()))?;

        Ok(())
    }

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let chat_id: i64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid chat ID".to_string()))?;
        let msg_id: i32 = message_id
            .parse()
            .map_err(|_| Error::Parse("Invalid message ID".to_string()))?;

        self.bot
            .delete_message(ChatId(chat_id), MessageId(msg_id))
            .await
            .map_err(|e| Error::Telegram(e.to_string()))?;

        Ok(())
    }

    async fn send_typing(&self, channel_id: &str) -> Result<()> {
        let chat_id: i64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid chat ID".to_string()))?;

        self.bot
            .send_chat_action(ChatId(chat_id), ChatAction::Typing)
            .await
            .map_err(|e| Error::Telegram(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_config() {
        let config = TelegramConfig::new("test_token")
            .with_allowed_users(vec![123, 456])
            .with_groups_mention_only(false);

        assert_eq!(config.bot_token, "test_token");
        assert_eq!(config.allowed_users, vec![123, 456]);
        assert!(!config.groups_mention_only);
    }

    #[test]
    fn test_build_keyboard() {
        let buttons = vec![
            MessageButton::callback("Yes", "approve:yes"),
            MessageButton::callback("No", "approve:no"),
        ];

        let keyboard = TelegramAdapter::build_keyboard(&buttons);
        assert!(keyboard.is_some());
    }

    #[test]
    fn test_user_allowed() {
        let config = TelegramConfig::new("token").with_allowed_users(vec![123, 456]);
        let adapter = TelegramAdapter::new(config);

        assert!(adapter.is_user_allowed(123));
        assert!(adapter.is_user_allowed(456));
        assert!(!adapter.is_user_allowed(789));
    }

    #[test]
    fn test_empty_allowlist_allows_all() {
        let config = TelegramConfig::new("token");
        let adapter = TelegramAdapter::new(config);

        assert!(adapter.is_user_allowed(123));
        assert!(adapter.is_user_allowed(999999));
    }

    #[test]
    fn test_dm_policy_default() {
        let config = TelegramConfig::new("token");
        assert_eq!(config.dm_policy, DmPolicy::Allowlist);
    }

    #[test]
    fn test_dm_policy_builder() {
        let config = TelegramConfig::new("token").with_dm_policy(DmPolicy::Open);
        assert_eq!(config.dm_policy, DmPolicy::Open);
    }

    #[test]
    fn test_dm_policy_disabled() {
        let config = TelegramConfig::new("token").with_dm_policy(DmPolicy::Disabled);
        assert_eq!(config.dm_policy, DmPolicy::Disabled);
    }

    // Note: mask_for_logging and sanitize_error_for_user tests are in util.rs
}
