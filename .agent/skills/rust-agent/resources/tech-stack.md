# Cratos 기술 스택

## 핵심 크레이트

### 런타임 & 웹

| 크레이트 | 버전 | 용도 | 예시 |
|---------|------|------|------|
| tokio | 1.x | 비동기 런타임 | `#[tokio::main]` |
| axum | 0.7 | HTTP API | `Router::new().route(...)` |
| tower | 0.4 | 미들웨어 | `ServiceBuilder::new()` |
| tower-http | 0.5 | HTTP 미들웨어 | `TraceLayer`, `CorsLayer` |

### 데이터베이스

| 크레이트 | 버전 | 용도 | 예시 |
|---------|------|------|------|
| sqlx | 0.8 | PostgreSQL | `sqlx::query_as!()` |
| redis | 0.26 | 캐시/세션 | `redis::Client::open()` |

### 채널 연동

| 크레이트 | 버전 | 용도 | 예시 |
|---------|------|------|------|
| teloxide | 0.12 | Telegram | `Bot::from_env()` |
| slack-morphism | 2.x | Slack | `SlackClient::new()` |

### LLM 연동

| 크레이트 | 버전 | 용도 | 예시 |
|---------|------|------|------|
| async-openai | 0.23 | OpenAI | `Client::new()` |
| reqwest | 0.12 | HTTP (Anthropic) | `Client::builder()` |

### 유틸리티

| 크레이트 | 버전 | 용도 | 예시 |
|---------|------|------|------|
| serde | 1.x | 직렬화 | `#[derive(Serialize)]` |
| serde_json | 1.x | JSON | `serde_json::to_string()` |
| tracing | 0.1 | 로깅 | `#[instrument]` |
| tracing-subscriber | 0.3 | 로그 출력 | `fmt::init()` |
| thiserror | 1.x | 에러 정의 | `#[derive(Error)]` |
| anyhow | 1.x | 에러 전파 | `anyhow::Result` |
| uuid | 1.x | UUID | `Uuid::new_v4()` |
| chrono | 0.4 | 시간 | `Utc::now()` |
| config | 0.14 | 설정 | `Config::builder()` |
| clap | 4.x | CLI | `#[derive(Parser)]` |

### Git & GitHub

| 크레이트 | 버전 | 용도 | 예시 |
|---------|------|------|------|
| git2 | 0.19 | Git 작업 | `Repository::open()` |
| octocrab | 0.39 | GitHub API | `Octocrab::builder()` |

## Cargo.toml 템플릿

```toml
[package]
name = "cratos-{crate}"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
thiserror = "1"
anyhow = "1"

[dev-dependencies]
tokio-test = "0.4"
```
