# Telegram 연동 가이드

## teloxide 기본 설정

```rust
use teloxide::prelude::*;

#[tokio::main]
async fn main() {
    let bot = Bot::from_env();

    teloxide::repl(bot, |bot: Bot, msg: Message| async move {
        bot.send_message(msg.chat.id, "Hello!").await?;
        Ok(())
    })
    .await;
}
```

## 메시지 핸들러 구조

```rust
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

type MyDialogue = Dialogue<State, InMemStorage<State>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    Processing,
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            // 정규화
            let normalized = normalize_telegram_message(&msg);

            // 처리
            let response = process_message(normalized).await?;

            // 응답
            bot.send_message(msg.chat.id, response).await?;
        }
        None => {
            bot.send_message(msg.chat.id, "텍스트 메시지만 지원합니다.")
                .await?;
        }
    }
    Ok(())
}
```

## 정규화 함수

```rust
fn normalize_telegram_message(msg: &Message) -> NormalizedMessage {
    NormalizedMessage {
        id: Uuid::new_v4(),
        channel: Channel::Telegram {
            chat_id: msg.chat.id.0,
        },
        workspace_id: format!("telegram:{}", msg.chat.id.0),
        user_id: msg.from().map(|u| u.id.0.to_string()).unwrap_or_default(),
        thread_id: msg.thread_id.map(|t| t.to_string()),
        message_id: msg.id.0.to_string(),
        timestamp: Utc::now(),
        text: msg.text().unwrap_or_default().to_string(),
        attachments: extract_attachments(msg),
    }
}
```

## 레이트리밋 처리

```rust
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

// Telegram: 30 messages/second (global), 1 message/second (per chat)
let limiter = RateLimiter::direct(Quota::per_second(NonZeroU32::new(30).unwrap()));

async fn send_with_limit(bot: &Bot, chat_id: ChatId, text: &str) -> Result<()> {
    limiter.until_ready().await;
    bot.send_message(chat_id, text).await?;
    Ok(())
}
```

## 환경 변수

```bash
TELOXIDE_TOKEN=your_bot_token
```
