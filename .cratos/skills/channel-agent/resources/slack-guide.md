# Slack 연동 가이드

## slack-morphism 기본 설정

```rust
use slack_morphism::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SlackClient::new(SlackClientHyperConnector::new()?);

    let token = SlackApiToken::new(
        std::env::var("SLACK_BOT_TOKEN")?.into()
    );

    let session = client.open_session(&token);

    Ok(())
}
```

## 이벤트 핸들러

```rust
use axum::{Router, routing::post, Json};
use slack_morphism::prelude::*;

async fn handle_slack_event(
    Json(event): Json<SlackPushEvent>,
) -> impl IntoResponse {
    match event {
        SlackPushEvent::EventCallback(callback) => {
            if let SlackEventCallbackBody::Message(msg) = callback.event {
                let normalized = normalize_slack_message(&msg);
                let response = process_message(normalized).await;
                // 응답 전송
            }
        }
        SlackPushEvent::UrlVerification(challenge) => {
            return Json(json!({ "challenge": challenge.challenge }));
        }
        _ => {}
    }

    Json(json!({}))
}
```

## 정규화 함수

```rust
fn normalize_slack_message(msg: &SlackMessageEvent) -> NormalizedMessage {
    NormalizedMessage {
        id: Uuid::new_v4(),
        channel: Channel::Slack {
            channel_id: msg.channel.clone().unwrap_or_default().to_string(),
            team_id: msg.team.clone().unwrap_or_default().to_string(),
        },
        workspace_id: msg.team.clone().unwrap_or_default().to_string(),
        user_id: msg.user.clone().unwrap_or_default().to_string(),
        thread_id: msg.thread_ts.clone().map(|t| t.to_string()),
        message_id: msg.ts.clone().unwrap_or_default().to_string(),
        timestamp: Utc::now(),
        text: msg.text.clone().unwrap_or_default(),
        attachments: extract_slack_attachments(msg),
    }
}
```

## 메시지 전송

```rust
async fn send_slack_message(
    session: &SlackClientSession<'_>,
    channel: &str,
    text: &str,
    thread_ts: Option<&str>,
) -> Result<(), SlackClientError> {
    let request = SlackApiChatPostMessageRequest::new(
        channel.into(),
        SlackMessageContent::new().with_text(text.into()),
    )
    .opt_thread_ts(thread_ts.map(|t| t.into()));

    session.chat_post_message(&request).await?;
    Ok(())
}
```

## 서명 검증

```rust
use slack_morphism::signature_verifier::SlackEventSignatureVerifier;

async fn verify_slack_signature(
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(), Error> {
    let verifier = SlackEventSignatureVerifier::new(
        &std::env::var("SLACK_SIGNING_SECRET")?
    )?;

    let timestamp = headers
        .get("x-slack-request-timestamp")
        .ok_or(Error::MissingHeader)?
        .to_str()?;

    let signature = headers
        .get("x-slack-signature")
        .ok_or(Error::MissingHeader)?
        .to_str()?;

    verifier.verify(timestamp, body, signature)?;
    Ok(())
}
```

## 환경 변수

```bash
SLACK_BOT_TOKEN=xoxb-your-token
SLACK_SIGNING_SECRET=your-signing-secret
SLACK_APP_TOKEN=xapp-your-app-token  # Socket Mode용
```
