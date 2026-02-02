# Cratos Rust 코딩 규칙

## 필수 크레이트

| 크레이트 | 용도 | 버전 |
|---------|------|------|
| tokio | 비동기 런타임 | 1.x |
| axum | HTTP API | 0.7 |
| sqlx | PostgreSQL | 0.8 |
| serde + serde_json | 직렬화 | 1.x |
| tracing | 로깅 | 0.1 |
| thiserror | 에러 정의 | 1.x |
| anyhow | 에러 전파 | 1.x |
| teloxide | Telegram Bot | 0.12 |
| slack-morphism | Slack SDK | 2.x |

## 코드 스타일

1. `#![forbid(unsafe_code)]` - unsafe 금지
2. `Result<T, E>` 명시적 반환
3. `?` 연산자로 에러 전파
4. `#[instrument]` 함수 추적
5. 700줄 이하 파일 유지

## 에러 처리

```rust
// thiserror for library errors
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Failed to send message: {0}")]
    SendFailed(String),
    #[error("Rate limit exceeded")]
    RateLimited,
}

// anyhow for application errors
pub async fn process() -> anyhow::Result<()> {
    let result = some_operation().await?;
    Ok(())
}
```

## 테스트

- 단위: `#[tokio::test]`
- 통합: `tests/*.rs`
- 커버리지: 70% 이상
- Mock: `mockall` 크레이트

## 커밋

- Conventional Commits 형식
- `Co-Authored-By: Claude <noreply@anthropic.com>`

## 모듈 구조

```
crate/
├── src/
│   ├── lib.rs          # 공개 API
│   ├── error.rs        # 에러 타입
│   ├── config.rs       # 설정
│   └── {module}/
│       ├── mod.rs
│       └── ...
└── tests/
    └── integration.rs
```
