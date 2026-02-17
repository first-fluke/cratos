# Cratos 기술 스택

## 크레이트 구조 (11개)

```
cratos/
├── crates/
│   ├── cratos-core/      # 핵심 오케스트레이션, 보안, 자격증명
│   ├── cratos-channels/  # 채널 어댑터 (Telegram, Slack, Discord, Matrix, WhatsApp)
│   ├── cratos-tools/     # 도구 레지스트리, 샌드박스
│   ├── cratos-llm/       # LLM 프로바이더 (13개)
│   ├── cratos-replay/    # 리플레이 엔진 (SQLite)
│   ├── cratos-skills/    # 자동 스킬 생성 시스템
│   ├── cratos-search/    # 벡터 검색, 시맨틱 인덱싱
│   ├── cratos-memory/    # Graph RAG 대화 메모리
│   ├── cratos-crypto/    # 암호화, 키 관리
│   ├── cratos-audio/     # 음성 제어 (STT/TTS)
│   └── cratos-canvas/    # 캔버스 (향후)
```

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
| sqlx | 0.8 | **SQLite** (embedded) | `sqlx::query_as!()` |
| redis | 1.0 | 캐시/세션 (선택) | `redis::Client::open()` |

> **참고**: Cratos는 PostgreSQL이 아닌 **SQLite**를 사용합니다 (경량 설치, 단일 바이너리).

### 채널 연동

| 크레이트 | 버전 | 용도 | 예시 |
|---------|------|------|------|
| teloxide | 0.17 | Telegram | `Bot::from_env()` |
| slack-morphism | 2.x | Slack | `SlackClient::new()` |
| serenity | 0.12 | Discord | `Client::builder()` |
| matrix-sdk | 0.10 | Matrix | `Client::builder()` |

### LLM 연동

| 크레이트 | 버전 | 용도 | 예시 |
|---------|------|------|------|
| async-openai | 0.32 | OpenAI | `Client::new()` |
| reqwest | 0.12 | HTTP (Anthropic, Gemini 등) | `Client::builder()` |

### ML/임베딩

| 크레이트 | 버전 | 용도 | 예시 |
|---------|------|------|------|
| tract-onnx | 0.22 | ONNX 임베딩 | `tract_onnx::onnx()` |
| candle-core | 0.8 | 텐서 연산 | `Tensor::zeros()` |
| hf-hub | 0.4 | HuggingFace 모델 | `Api::new()` |
| usearch | 2.x | 벡터 인덱스 | `Index::new()` |

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
rust-version = "1.88"

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
