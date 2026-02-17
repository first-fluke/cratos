# Telegram 연동 가이드

## TelegramAdapter 아키텍처

Cratos는 `TelegramAdapter` 클래스를 사용합니다 (`crates/cratos-channels/src/telegram/`).

```rust
pub struct TelegramAdapter {
    config: TelegramConfig,
    bot: Bot,
    orchestrator: Arc<Orchestrator>,
    dev_monitor: Option<Arc<DevSessionMonitor>>,
}

impl TelegramAdapter {
    pub async fn run(
        orchestrator: Arc<Orchestrator>,
        dev_monitor: Option<Arc<DevSessionMonitor>>,
    ) -> Result<()> {
        // ...
    }
}
```

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

## 슬래시 명령어 (Cratos 구현)

| 명령어 | 설명 |
|--------|------|
| `/status` | 현재 실행 상태 |
| `/sessions` | 활성 AI 세션 목록 |
| `/tools` | 사용 가능한 도구 목록 |
| `/cancel` | 실행 취소 |
| `/approve` | 승인 대기 작업 승인 |

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

## 마크다운 → HTML 변환 (중요!)

Telegram의 MarkdownV2 파싱은 까다롭습니다. **HTML ParseMode** 사용 권장:

```rust
use crate::util::markdown_to_html;

// 응답 전송
bot.send_message(chat_id, markdown_to_html(&response))
    .parse_mode(ParseMode::Html)
    .await?;
```

`markdown_to_html()` 함수 (`util.rs`):
- `**bold**` → `<b>bold</b>`
- `*italic*` → `<i>italic</i>`
- `` `code` `` → `<code>code</code>`
- `[text](url)` → `<a href="url">text</a>`

## 진행 메시지 레이스 해결

```rust
// 진행 메시지 전송
let progress_msg = bot.send_message(chat_id, "처리 중...").await?;

// 작업 완료 후 진행 메시지 삭제
bot.delete_message(chat_id, progress_msg.id).await.ok();

// 새 메시지로 최종 응답 전송 (edit_message_text 대신)
bot.send_message(chat_id, final_response).await?;
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

## TelegramConfig

```rust
pub struct TelegramConfig {
    pub token: String,
    pub dm_policy: DmPolicy,
    pub notify_chat_id: Option<i64>,
    pub allowed_users: Vec<i64>,
}

pub enum DmPolicy {
    Allow,
    Deny,
    AllowListed,
}
```

## 환경 변수

```bash
TELOXIDE_TOKEN=your_bot_token
```
