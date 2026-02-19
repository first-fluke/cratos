---
name: Slack Integration
description: This skill should be used when implementing Slack app functionality using slack-morphism.
version: 1.0.0
---

# Slack Integration

slack-morphism를 사용한 Slack App 연동.

## 기본 설정

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

## 환경 변수

```bash
SLACK_BOT_TOKEN=xoxb-your-token
SLACK_SIGNING_SECRET=your-signing-secret
SLACK_APP_TOKEN=xapp-your-app-token
```

## 이벤트 처리

```rust
async fn handle_slack_event(
    Json(event): Json<SlackPushEvent>,
) -> impl IntoResponse {
    match event {
        SlackPushEvent::EventCallback(callback) => {
            if let SlackEventCallbackBody::Message(msg) = callback.event {
                let normalized = normalize_slack_message(&msg);
                // 처리
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

## 서명 검증

```rust
let verifier = SlackEventSignatureVerifier::new(
    &std::env::var("SLACK_SIGNING_SECRET")?
)?;
verifier.verify(timestamp, body, signature)?;
```

## 참조

- `.cratos/skills/channel-agent/resources/slack-guide.md`
