use super::{constant_time_eq, SlackAdapter, MAX_TIMESTAMP_AGE_SECS};
use crate::error::{Error, Result};
use crate::message::{ChannelAdapter, ChannelType, OutgoingAttachment, OutgoingMessage};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use slack_morphism::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

impl SlackAdapter {
    /// Verify a Slack request signature (HMAC-SHA256)
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
    pub fn verify_webhook_request(&self, headers: &[(String, String)], body: &str) -> Result<()> {
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
    pub(crate) async fn fetch_bot_info(&self) -> Result<()> {
        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::Slack(format!("Failed to create HTTP connector: {}", e)))?;
        let client = SlackClient::new(connector);
        let token = self.bot_token();
        let session = client.open_session(&token);

        let auth_response = session
            .auth_test()
            .await
            .map_err(|e| Error::Slack(format!("Failed to fetch bot info: {}", e)))?;

        let user_id = auth_response.user_id;
        info!(user_id = %user_id, "Bot user ID fetched");
        self.set_bot_user_id(user_id.to_string()).await;

        Ok(())
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

    async fn send_attachment(
        &self,
        channel_id: &str,
        attachment: OutgoingAttachment,
        reply_to: Option<&str>,
    ) -> Result<String> {
        // Decode base64 attachment data
        use base64::Engine as _;
        let file_data = base64::engine::general_purpose::STANDARD
            .decode(&attachment.data)
            .map_err(|e| Error::Slack(format!("Invalid base64 attachment data: {}", e)))?;

        let http_client = reqwest::Client::new();

        // Step 1: Get upload URL from Slack
        let upload_url_response = http_client
            .post("https://slack.com/api/files.getUploadURLExternal")
            .bearer_auth(&self.config.bot_token)
            .form(&[
                ("filename", attachment.filename.as_str()),
                ("length", &file_data.len().to_string()),
            ])
            .send()
            .await
            .map_err(|e| Error::Slack(format!("Failed to get upload URL: {}", e)))?;

        let url_data: serde_json::Value = upload_url_response
            .json()
            .await
            .map_err(|e| Error::Slack(format!("Failed to parse upload URL response: {}", e)))?;

        if !url_data["ok"].as_bool().unwrap_or(false) {
            let error_msg = url_data["error"].as_str().unwrap_or("Unknown error");
            return Err(Error::Slack(format!(
                "Failed to get upload URL: {}",
                error_msg
            )));
        }

        let upload_url = url_data["upload_url"]
            .as_str()
            .ok_or_else(|| Error::Slack("No upload_url in response".to_string()))?;
        let file_id = url_data["file_id"]
            .as_str()
            .ok_or_else(|| Error::Slack("No file_id in response".to_string()))?;

        debug!(file_id = %file_id, "Got Slack upload URL");

        // Step 2: Upload file content to the external URL
        http_client
            .put(upload_url)
            .header("Content-Type", &attachment.mime_type)
            .body(file_data)
            .send()
            .await
            .map_err(|e| Error::Slack(format!("Failed to upload file: {}", e)))?;

        debug!(file_id = %file_id, "File uploaded to Slack");

        // Step 3: Complete the upload and share to channel
        let title = attachment
            .caption
            .as_deref()
            .unwrap_or(&attachment.filename);

        let files_json = serde_json::json!([{
            "id": file_id,
            "title": title
        }]);

        let mut form_params = vec![
            ("files", files_json.to_string()),
            ("channel_id", channel_id.to_string()),
        ];

        if let Some(thread_ts) = reply_to {
            form_params.push(("thread_ts", thread_ts.to_string()));
        }

        // Add initial comment if caption is different from filename
        if let Some(caption) = &attachment.caption {
            if !caption.is_empty() && caption != &attachment.filename {
                form_params.push(("initial_comment", caption.clone()));
            }
        }

        let complete_response = http_client
            .post("https://slack.com/api/files.completeUploadExternal")
            .bearer_auth(&self.config.bot_token)
            .form(&form_params)
            .send()
            .await
            .map_err(|e| Error::Slack(format!("Failed to complete upload: {}", e)))?;

        let complete_data: serde_json::Value = complete_response
            .json()
            .await
            .map_err(|e| Error::Slack(format!("Failed to parse complete response: {}", e)))?;

        if !complete_data["ok"].as_bool().unwrap_or(false) {
            let error_msg = complete_data["error"].as_str().unwrap_or("Unknown error");
            return Err(Error::Slack(format!(
                "Failed to complete upload: {}",
                error_msg
            )));
        }

        info!(
            file_id = %file_id,
            filename = %attachment.filename,
            channel = %channel_id,
            "File uploaded to Slack"
        );

        // Return the file ID as the message identifier
        Ok(file_id.to_string())
    }
}
