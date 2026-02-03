//! Slack - slack-morphism adapter
//!
//! This module provides the Slack bot adapter using the slack-morphism library
//! with Socket Mode support for real-time event handling.

use crate::error::{Error, Result};
use crate::message::{
    ChannelAdapter, ChannelType, MessageButton, NormalizedMessage, OutgoingMessage,
};
use cratos_core::{Orchestrator, OrchestratorInput};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use slack_morphism::prelude::*;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

/// Maximum allowed timestamp age in seconds (5 minutes)
const MAX_TIMESTAMP_AGE_SECS: u64 = 300;

/// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Slack bot configuration
#[derive(Debug, Clone)]
pub struct SlackConfig {
    /// Bot token (xoxb-...)
    pub bot_token: String,
    /// App token for Socket Mode (xapp-...)
    pub app_token: String,
    /// Signing secret for request verification
    pub signing_secret: String,
    /// Allowed workspace IDs (empty = allow all)
    pub allowed_workspaces: Vec<String>,
    /// Allowed channel IDs (empty = allow all)
    pub allowed_channels: Vec<String>,
    /// Whether to respond only to mentions/direct messages
    pub mentions_only: bool,
}

impl SlackConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let bot_token = std::env::var("SLACK_BOT_TOKEN")
            .map_err(|_| Error::Slack("SLACK_BOT_TOKEN not set".to_string()))?;

        let app_token = std::env::var("SLACK_APP_TOKEN")
            .map_err(|_| Error::Slack("SLACK_APP_TOKEN not set".to_string()))?;

        let signing_secret = std::env::var("SLACK_SIGNING_SECRET")
            .map_err(|_| Error::Slack("SLACK_SIGNING_SECRET not set".to_string()))?;

        let allowed_workspaces: Vec<String> = std::env::var("SLACK_ALLOWED_WORKSPACES")
            .ok()
            .map(|s| s.split(',').map(|id| id.trim().to_string()).collect())
            .unwrap_or_default();

        let allowed_channels: Vec<String> = std::env::var("SLACK_ALLOWED_CHANNELS")
            .ok()
            .map(|s| s.split(',').map(|id| id.trim().to_string()).collect())
            .unwrap_or_default();

        let mentions_only = std::env::var("SLACK_MENTIONS_ONLY")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(true);

        Ok(Self {
            bot_token,
            app_token,
            signing_secret,
            allowed_workspaces,
            allowed_channels,
            mentions_only,
        })
    }

    /// Create with a bot token
    #[must_use]
    pub fn new(
        bot_token: impl Into<String>,
        app_token: impl Into<String>,
        signing_secret: impl Into<String>,
    ) -> Self {
        Self {
            bot_token: bot_token.into(),
            app_token: app_token.into(),
            signing_secret: signing_secret.into(),
            allowed_workspaces: Vec::new(),
            allowed_channels: Vec::new(),
            mentions_only: true,
        }
    }

    /// Set allowed workspaces
    #[must_use]
    pub fn with_allowed_workspaces(mut self, workspaces: Vec<String>) -> Self {
        self.allowed_workspaces = workspaces;
        self
    }

    /// Set allowed channels
    #[must_use]
    pub fn with_allowed_channels(mut self, channels: Vec<String>) -> Self {
        self.allowed_channels = channels;
        self
    }

    /// Set mentions only mode
    #[must_use]
    pub fn with_mentions_only(mut self, enabled: bool) -> Self {
        self.mentions_only = enabled;
        self
    }
}

/// Slack bot adapter with Socket Mode support
pub struct SlackAdapter {
    config: SlackConfig,
    bot_user_id: RwLock<Option<String>>,
}

impl SlackAdapter {
    /// Create a new Slack adapter
    #[must_use]
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            bot_user_id: RwLock::new(None),
        }
    }

    /// Create from environment
    pub fn from_env() -> Result<Self> {
        let config = SlackConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Get the bot token
    fn bot_token(&self) -> SlackApiToken {
        SlackApiToken::new(self.config.bot_token.clone().into())
    }

    /// Get the app token (for socket mode)
    #[allow(dead_code)]
    fn app_token(&self) -> SlackApiToken {
        SlackApiToken::new(self.config.app_token.clone().into())
    }

    /// Get the cached bot user ID
    pub async fn get_bot_user_id(&self) -> Option<String> {
        self.bot_user_id.read().await.clone()
    }

    /// Set the bot user ID
    async fn set_bot_user_id(&self, user_id: String) {
        let mut guard = self.bot_user_id.write().await;
        *guard = Some(user_id);
    }

    /// Check if a workspace is allowed
    pub fn is_workspace_allowed(&self, workspace_id: &str) -> bool {
        self.config.allowed_workspaces.is_empty()
            || self
                .config
                .allowed_workspaces
                .contains(&workspace_id.to_string())
    }

    /// Check if a channel is allowed
    pub fn is_channel_allowed(&self, channel_id: &str) -> bool {
        self.config.allowed_channels.is_empty()
            || self
                .config
                .allowed_channels
                .contains(&channel_id.to_string())
    }

    /// Check if the message mentions the bot
    pub async fn is_bot_mentioned(&self, text: &str) -> bool {
        if let Some(bot_id) = self.get_bot_user_id().await {
            text.contains(&format!("<@{}>", bot_id))
        } else {
            false
        }
    }

    /// Verify a Slack request signature (HMAC-SHA256)
    ///
    /// This implements Slack's request signing verification as described at:
    /// https://api.slack.com/authentication/verifying-requests-from-slack
    ///
    /// # Arguments
    /// * `timestamp` - The X-Slack-Request-Timestamp header value
    /// * `body` - The raw request body
    /// * `signature` - The X-Slack-Signature header value (v0=...)
    ///
    /// # Returns
    /// * `Ok(())` if the signature is valid
    /// * `Err(...)` if verification fails
    pub fn verify_signature(&self, timestamp: &str, body: &str, signature: &str) -> Result<()> {
        // Check timestamp to prevent replay attacks
        let ts: u64 = timestamp
            .parse()
            .map_err(|_| Error::Slack("Invalid timestamp".to_string()))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Slack("System time error".to_string()))?
            .as_secs();

        if now.abs_diff(ts) > MAX_TIMESTAMP_AGE_SECS {
            warn!(
                timestamp = %ts,
                now = %now,
                "Slack request timestamp too old (possible replay attack)"
            );
            return Err(Error::Slack(
                "Request timestamp is too old or in the future".to_string(),
            ));
        }

        // Compute expected signature
        let sig_basestring = format!("v0:{}:{}", timestamp, body);

        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(self.config.signing_secret.as_bytes())
            .map_err(|_| Error::Slack("Invalid signing secret".to_string()))?;
        mac.update(sig_basestring.as_bytes());
        let expected = mac.finalize().into_bytes();
        let expected_hex = format!("v0={}", hex::encode(expected));

        // Constant-time comparison to prevent timing attacks
        if !constant_time_eq(signature.as_bytes(), expected_hex.as_bytes()) {
            warn!("Slack signature verification failed");
            return Err(Error::Slack("Invalid request signature".to_string()));
        }

        debug!("Slack signature verified successfully");
        Ok(())
    }

    /// Verify webhook request with all headers
    pub fn verify_webhook_request(
        &self,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<()> {
        let timestamp = headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == "x-slack-request-timestamp")
            .map(|(_, v)| v.as_str())
            .ok_or_else(|| Error::Slack("Missing X-Slack-Request-Timestamp header".to_string()))?;

        let signature = headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == "x-slack-signature")
            .map(|(_, v)| v.as_str())
            .ok_or_else(|| Error::Slack("Missing X-Slack-Signature header".to_string()))?;

        self.verify_signature(timestamp, body, signature)
    }

    /// Fetch bot user info and cache the bot user ID
    async fn fetch_bot_info(&self) -> Result<()> {
        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::Slack(format!("Failed to create HTTP connector: {}", e)))?;
        let client = SlackClient::new(connector);
        let token = self.bot_token();
        let session = client.open_session(&token);

        let auth_response = session
            .auth_test()
            .await
            .map_err(|e| Error::Slack(format!("Failed to fetch bot info: {}", e)))?;

        // user_id is directly a SlackUserId, not Option
        let user_id = auth_response.user_id;
        info!(user_id = %user_id, "Bot user ID fetched");
        self.set_bot_user_id(user_id.to_string()).await;

        Ok(())
    }

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

    /// Start the bot with the given orchestrator
    ///
    /// Note: Full Socket Mode implementation requires additional setup.
    /// This implementation uses a polling approach for simplicity.
    /// For production, consider implementing the full socket mode client.
    #[instrument(skip(self, _orchestrator))]
    pub async fn run(self: Arc<Self>, _orchestrator: Arc<Orchestrator>) -> Result<()> {
        info!("Starting Slack adapter");

        // Fetch bot info first
        self.fetch_bot_info().await?;

        info!(
            bot_user_id = ?self.get_bot_user_id().await,
            "Slack adapter ready"
        );

        // Note: Full Socket Mode requires implementing SlackSocketModeClientListener
        // and using the socket_mode feature. For now, we log that we're ready.
        // The actual message handling happens through the ChannelAdapter trait
        // when messages are routed from a webhook or another integration.
        //
        // To implement full Socket Mode:
        // 1. Enable the socket_mode feature in slack-morphism
        // 2. Implement SlackSocketModeClientListener
        // 3. Use SlackClientSocketModeConfig and register with the client
        //
        // For this implementation, we'll wait indefinitely as a placeholder
        // for the real socket mode connection.

        info!("Slack adapter is running. Waiting for shutdown signal...");

        // Keep running until shutdown
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| Error::Slack(format!("Signal error: {}", e)))?;

        info!("Slack adapter shutdown complete");
        Ok(())
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

    /// Build Slack blocks from buttons
    #[allow(dead_code)]
    fn build_blocks(text: &str, buttons: &[MessageButton]) -> Vec<SlackBlock> {
        let mut blocks = vec![SlackBlock::Section(SlackSectionBlock::new().with_text(
            SlackBlockText::MarkDown(SlackBlockMarkDownText::new(text.to_string())),
        ))];

        if !buttons.is_empty() {
            let button_elements: Vec<SlackActionBlockElement> = buttons
                .iter()
                .filter_map(|b| {
                    b.callback_data.as_ref().map(|callback_data| {
                        SlackActionBlockElement::Button(SlackBlockButtonElement::new(
                            callback_data.clone().into(),
                            SlackBlockPlainTextOnly::from(b.text.clone()),
                        ))
                    })
                })
                .collect();

            if !button_elements.is_empty() {
                blocks.push(SlackBlock::Actions(SlackActionsBlock::new(button_elements)));
            }
        }

        blocks
    }
}

#[async_trait::async_trait]
impl ChannelAdapter for SlackAdapter {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Slack
    }

    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String> {
        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::Slack(format!("Failed to create HTTP connector: {}", e)))?;
        let client = SlackClient::new(connector);
        let token = self.bot_token();
        let session = client.open_session(&token);

        let content = SlackMessageContent::new().with_text(message.text.clone());

        let mut request = SlackApiChatPostMessageRequest::new(channel_id.into(), content);

        // Set thread_ts for replies
        if let Some(thread_id) = &message.thread_id {
            request = request.with_thread_ts(thread_id.clone().into());
        }

        let response = session
            .chat_post_message(&request)
            .await
            .map_err(|e| Error::Slack(format!("Failed to send message: {}", e)))?;

        Ok(response.ts.to_string())
    }

    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        message: OutgoingMessage,
    ) -> Result<()> {
        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::Slack(format!("Failed to create HTTP connector: {}", e)))?;
        let client = SlackClient::new(connector);
        let token = self.bot_token();
        let session = client.open_session(&token);

        let content = SlackMessageContent::new().with_text(message.text.clone());

        let request = SlackApiChatUpdateRequest::new(channel_id.into(), content, message_id.into());

        session
            .chat_update(&request)
            .await
            .map_err(|e| Error::Slack(format!("Failed to update message: {}", e)))?;

        Ok(())
    }

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::Slack(format!("Failed to create HTTP connector: {}", e)))?;
        let client = SlackClient::new(connector);
        let token = self.bot_token();
        let session = client.open_session(&token);

        let request = SlackApiChatDeleteRequest::new(channel_id.into(), message_id.into());

        session
            .chat_delete(&request)
            .await
            .map_err(|e| Error::Slack(format!("Failed to delete message: {}", e)))?;

        Ok(())
    }

    async fn send_typing(&self, _channel_id: &str) -> Result<()> {
        // Slack doesn't have a typing indicator API for bots
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_config() {
        let config = SlackConfig::new("xoxb-test", "xapp-test", "signing-secret")
            .with_allowed_channels(vec!["C123".to_string()])
            .with_mentions_only(false);

        assert_eq!(config.bot_token, "xoxb-test");
        assert_eq!(config.allowed_channels, vec!["C123".to_string()]);
        assert!(!config.mentions_only);
    }

    #[test]
    fn test_channel_allowed() {
        let config = SlackConfig::new("xoxb-test", "xapp-test", "secret")
            .with_allowed_channels(vec!["C123".to_string(), "C456".to_string()]);
        let adapter = SlackAdapter::new(config);

        assert!(adapter.is_channel_allowed("C123"));
        assert!(adapter.is_channel_allowed("C456"));
        assert!(!adapter.is_channel_allowed("C789"));
    }

    #[test]
    fn test_empty_allowlist_allows_all() {
        let config = SlackConfig::new("xoxb-test", "xapp-test", "secret");
        let adapter = SlackAdapter::new(config);

        assert!(adapter.is_channel_allowed("C123"));
        assert!(adapter.is_channel_allowed("ANY_CHANNEL"));
    }

    #[test]
    fn test_workspace_allowed() {
        let config = SlackConfig::new("xoxb-test", "xapp-test", "secret")
            .with_allowed_workspaces(vec!["T123".to_string()]);
        let adapter = SlackAdapter::new(config);

        assert!(adapter.is_workspace_allowed("T123"));
        assert!(!adapter.is_workspace_allowed("T999"));
    }

    #[test]
    fn test_dm_channel_detection() {
        // DM channels in Slack start with 'D'
        assert!("D1234567890".starts_with('D'));
        assert!(!"C1234567890".starts_with('D'));
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
        assert!(!constant_time_eq(b"a", b"ab"));
    }

    #[test]
    fn test_signature_verification() {
        // Test with known values
        let config = SlackConfig::new("xoxb-test", "xapp-test", "8f742231b10e8888abcd99yyyzzz85a5");
        let adapter = SlackAdapter::new(config);

        // Use current timestamp to avoid replay protection rejection
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = now.to_string();
        let body = "token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J&team_domain=testteamnow&channel_id=G8PSS9T3V&channel_name=foobar&user_id=U2CERLKJA&user_name=roadrunner&command=%2Fwebhook-collect&text=&response_url=https%3A%2F%2Fhooks.slack.com%2Fcommands%2FT1DC2JH3J%2F397700885554%2F96rGlfmibIGlgcZRskXaIFfN&trigger_id=398738663015.47445629121.803a0bc887a14d10d2c447fce8b6703c";

        // Compute expected signature manually
        let sig_basestring = format!("v0:{}:{}", timestamp, body);
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;
        let mut mac =
            HmacSha256::new_from_slice(b"8f742231b10e8888abcd99yyyzzz85a5").unwrap();
        mac.update(sig_basestring.as_bytes());
        let expected = mac.finalize().into_bytes();
        let signature = format!("v0={}", hex::encode(expected));

        // Should verify successfully with correct signature
        assert!(adapter.verify_signature(&timestamp, body, &signature).is_ok());

        // Should fail with incorrect signature
        assert!(adapter
            .verify_signature(&timestamp, body, "v0=invalid")
            .is_err());
    }

    #[test]
    fn test_signature_replay_protection() {
        let config = SlackConfig::new("xoxb-test", "xapp-test", "secret");
        let adapter = SlackAdapter::new(config);

        // Very old timestamp should be rejected
        let result = adapter.verify_signature("1000000000", "body", "v0=sig");
        assert!(result.is_err());
    }
}
