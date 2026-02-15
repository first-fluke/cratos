//! ChannelAdapter trait implementation for Telegram

use super::adapter::TelegramAdapter;
use crate::error::{Error, Result};
use crate::message::{
    AttachmentType, ChannelAdapter, ChannelType, OutgoingAttachment, OutgoingMessage,
};
use crate::util::markdown_to_html;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use teloxide::{
    payloads::{SendDocumentSetters, SendMessageSetters, SendPhotoSetters},
    prelude::*,
    types::{ChatAction, ChatId, InputFile, MessageId, ParseMode, ReplyParameters},
};

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
            request = self
                .bot
                .send_message(ChatId(chat_id), &html_text)
                .parse_mode(ParseMode::Html);
        }

        if let Some(reply_to) = &message.reply_to {
            if let Ok(msg_id) = reply_to.parse::<i32>() {
                request = request.reply_parameters(ReplyParameters::new(MessageId(msg_id)));
            }
        }

        if let Some(keyboard) = TelegramAdapter::build_keyboard(&message.buttons) {
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
            request = self
                .bot
                .edit_message_text(ChatId(chat_id), MessageId(msg_id), &html_text)
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

    async fn send_attachment(
        &self,
        channel_id: &str,
        attachment: OutgoingAttachment,
        reply_to: Option<&str>,
    ) -> Result<String> {
        let chat_id: i64 = channel_id
            .parse()
            .map_err(|_| Error::Parse("Invalid chat ID".to_string()))?;

        // Decode base64 data
        let data = BASE64
            .decode(&attachment.data)
            .map_err(|e| Error::Parse(format!("Invalid base64 data: {}", e)))?;

        let input_file = InputFile::memory(data).file_name(attachment.filename.clone());
        let caption = attachment.caption.as_deref();

        // Build reply parameters if reply_to is provided
        let reply_params = reply_to
            .and_then(|r| r.parse::<i32>().ok())
            .map(|msg_id| ReplyParameters::new(MessageId(msg_id)));

        let sent = match attachment.attachment_type {
            AttachmentType::Image => {
                let mut request = self.bot.send_photo(ChatId(chat_id), input_file);
                if let Some(cap) = caption {
                    request = request.caption(cap);
                }
                if let Some(rp) = reply_params {
                    request = request.reply_parameters(rp);
                }
                request.await.map_err(|e| Error::Telegram(e.to_string()))?
            }
            AttachmentType::Audio | AttachmentType::Voice => {
                // For audio, we use send_document as send_audio requires specific metadata
                let mut request = self.bot.send_document(ChatId(chat_id), input_file);
                if let Some(cap) = caption {
                    request = request.caption(cap);
                }
                if let Some(rp) = reply_params {
                    request = request.reply_parameters(rp);
                }
                request.await.map_err(|e| Error::Telegram(e.to_string()))?
            }
            AttachmentType::Video => {
                // Videos sent as documents for simplicity
                let mut request = self.bot.send_document(ChatId(chat_id), input_file);
                if let Some(cap) = caption {
                    request = request.caption(cap);
                }
                if let Some(rp) = reply_params {
                    request = request.reply_parameters(rp);
                }
                request.await.map_err(|e| Error::Telegram(e.to_string()))?
            }
            AttachmentType::Document | AttachmentType::Other => {
                let mut request = self.bot.send_document(ChatId(chat_id), input_file);
                if let Some(cap) = caption {
                    request = request.caption(cap);
                }
                if let Some(rp) = reply_params {
                    request = request.reply_parameters(rp);
                }
                request.await.map_err(|e| Error::Telegram(e.to_string()))?
            }
        };

        Ok(sent.id.0.to_string())
    }
}
