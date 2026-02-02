---
name: Telegram Integration
description: This skill should be used when implementing Telegram bot functionality using teloxide.
version: 1.0.0
---

# Telegram Integration

teloxide를 사용한 Telegram Bot 연동.

## 기본 설정

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

## 환경 변수

```bash
TELOXIDE_TOKEN=your_bot_token
```

## 메시지 정규화

```rust
fn normalize_telegram_message(msg: &Message) -> NormalizedMessage {
    NormalizedMessage {
        id: Uuid::new_v4(),
        channel: Channel::Telegram { chat_id: msg.chat.id.0 },
        user_id: msg.from().map(|u| u.id.0.to_string()).unwrap_or_default(),
        text: msg.text().unwrap_or_default().to_string(),
        timestamp: Utc::now(),
    }
}
```

## 레이트리밋

```rust
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

// Telegram: 30 msg/sec (global), 1 msg/sec (per chat)
let limiter = RateLimiter::direct(
    Quota::per_second(NonZeroU32::new(30).unwrap())
);
```

## 참조

- `.agent/skills/channel-agent/resources/telegram-guide.md`
