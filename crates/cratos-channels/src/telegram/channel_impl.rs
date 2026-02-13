//! ChannelAdapter trait implementation for Telegram

use super::adapter::TelegramAdapter;
use crate::error::{Error, Result};
use crate::message::{ChannelAdapter, ChannelType, OutgoingMessage};
use crate::util::markdown_to_html;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    types::{ChatAction, ChatId, MessageId, ParseMode, ReplyParameters},
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
}
