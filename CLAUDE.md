# Cratos - AI-Powered Personal Assistant

## Quick Start

```bash
cargo build                      # 빌드
cargo build --profile dev-release -p cratos  # 최적화 빌드 (thin LTO, ~5분)
cargo build --release -p cratos              # 배포 빌드 (full LTO, ~10분)
cargo test --workspace           # 전체 테스트 (1,200+ tests)
cargo clippy --all-targets       # 린트
cargo check --all-targets        # 빠른 타입 체크
cratos init                      # 대화형 설정 마법사
cratos serve                     # 서버 시작 (기본 포트 19527)
cratos tui                       # 대화형 TUI 채팅
```

### 바이너리 배포

```bash
cp target/dev-release/cratos /usr/local/bin/   # 설치 (dev-release)
cp target/release/cratos /usr/local/bin/        # 설치 (release)
nohup cratos serve > /tmp/cratos-server.log 2>&1 &  # 백그라운드 서버
```

## 프로젝트 개요

Cratos는 Telegram/Slack/Discord에서 자연어로 명령을 내리면 AI 에이전트가 이해하고, 정보를 모으고, 실행하고, 결과를 보고하는 **Rust 기반 AI 어시스턴트**입니다.

### 핵심 기능

| 기능 | 설명 |
|------|------|
| **경량 설치** | Docker/PostgreSQL 불필요, SQLite 내장, 단일 바이너리 |
| **자동 스킬 생성** | 사용 패턴 학습 → 자동 스킬 생성 |
| **리플레이 (되감기)** | 타임라인 + 재실행 + dry-run |
| **모델 라우팅** | 작업별 자동 모델 선택, 비용 최적화 |
| **원격 개발 E2E** | Issue → PR 완전 자동화 (`cratos develop`) |
| **Tool Doctor** | 자기 진단 + 해결 체크리스트 |
| **비용/할당량 모니터링** | CostTracker + QuotaTracker, 프로바이더별 사용량 추적 |
| **TUI 채팅** | ratatui 기반 대화형 터미널 인터페이스 (설정 모달 F5) |
| **스케줄러** | ProactiveScheduler (Cron/Interval/OneTime) |
| **REST API + WebSocket** | 51개 REST 엔드포인트, `/ws/chat`, `/ws/events` 실시간 통신 |
| **음성 어시스턴트** | STT(Whisper) + TTS, 실시간 음성 대화 |
| **브라우저 제어** | Chrome 확장 + CDP 통합, 탭/스크린샷 제어 |
| **디바이스 페어링** | PIN 기반 E2E 암호화 디바이스 연동 |
| **ACP/MCP 브릿지** | IDE 통합 (stdin/stdout JSON-lines) |

## 기술 스택

- **언어**: Rust 1.88+ (`#![forbid(unsafe_code)]`)
- **런타임**: Tokio (비동기)
- **웹**: Axum 0.7
- **DB**: SQLite (sqlx, 내장), Redis (세션용, 선택)
- **채널**: teloxide (Telegram), slack-morphism (Slack), serenity (Discord), matrix-sdk (Matrix)
- **LLM**: 13개 프로바이더 (OpenAI, Anthropic, Gemini, DeepSeek, Groq, Fireworks, SiliconFlow, GLM, Qwen, Moonshot, Novita, OpenRouter, Ollama)
- **임베딩/검색**: tract-onnx (pure Rust ONNX 임베딩), usearch (벡터 인덱스)
- **오디오**: cpal, rodio, hound + candle 0.8.x (로컬 Whisper STT)
- **Web UI**: Leptos (WASM), Tailwind CSS
- **TUI**: ratatui 0.29, tui-textarea 0.7, tui-markdown =0.3.5

## 프로젝트 구조

```
cratos/
├── apps/
│   ├── web/                    # Leptos Web UI (WASM)
│   ├── android/                # Android 앱
│   ├── ios/                    # iOS 앱
│   └── desktop/                # 데스크톱 앱
├── assets/
│   ├── chrome-extension/       # Chrome 확장 (CDP 통합, WebSocket Gateway)
│   └── a2ui/                   # UI 자산
├── config/
│   ├── default.toml            # 기본 설정 (수정 금지)
│   ├── local.toml              # 사용자 오버라이드 (gitignored)
│   ├── pantheon/               # 페르소나 TOML (14개)
│   └── decrees/                # 율법, 계급, 개발 규칙
├── crates/                     # Rust workspace (11개 크레이트)
│   ├── cratos-core/            # 오케스트레이션, 보안, 자격증명, 이벤트버스
│   ├── cratos-channels/        # 채널 어댑터 (Telegram, Slack, Discord, Matrix)
│   ├── cratos-tools/           # 도구 레지스트리, 샌드박스, exec/bash
│   ├── cratos-llm/             # LLM 프로바이더 13개, 임베딩, 쿼터 추적
│   ├── cratos-replay/          # 리플레이 엔진 (SQLite 이벤트 스토어)
│   ├── cratos-skills/          # 자동 스킬 생성, 시맨틱 라우팅
│   ├── cratos-search/          # 벡터 검색, 시맨틱 인덱싱
│   ├── cratos-memory/          # Graph RAG 대화 메모리
│   ├── cratos-crypto/          # E2E 암호화, 키 관리
│   ├── cratos-audio/           # 음성 제어 (STT/TTS)
│   └── cratos-canvas/          # 캔버스 (future)
├── src/
│   ├── main.rs                 # 엔트리포인트
│   ├── cli/                    # CLI 모듈 (17개 서브커맨드)
│   │   ├── mod.rs              # Commands enum 정의
│   │   ├── setup/              # cratos init (대화형 마법사)
│   │   ├── tui/                # TUI 채팅 (app, event, ui, settings)
│   │   ├── config.rs           # cratos config {list,get,set,reset,edit}
│   │   ├── skill/              # cratos skill {list,show,enable,disable}
│   │   ├── data.rs             # cratos data {stats,clear}
│   │   ├── develop.rs          # cratos develop (Issue → PR)
│   │   ├── voice.rs            # cratos voice (음성 어시스턴트)
│   │   ├── browser_ext.rs      # cratos browser {extension,tabs,open,screenshot}
│   │   ├── pair.rs             # cratos pair {start,devices,unpair}
│   │   ├── security.rs         # cratos security audit
│   │   ├── doctor.rs           # cratos doctor
│   │   ├── quota.rs            # cratos quota
│   │   ├── pantheon.rs         # cratos pantheon
│   │   ├── chronicle.rs        # cratos chronicle
│   │   └── decrees.rs          # cratos decrees
│   ├── api/                    # REST API (17개 모듈, 51개 엔드포인트)
│   │   ├── config/             # GET/PUT /api/v1/config
│   │   ├── executions/         # /api/v1/executions/*
│   │   ├── sessions/           # /api/v1/sessions/*
│   │   ├── scheduler/          # /api/v1/scheduler/*
│   │   ├── skills.rs           # /api/v1/skills/*
│   │   ├── browser.rs          # /api/v1/browser/*
│   │   ├── pairing.rs          # /api/v1/pair/*
│   │   ├── graph.rs            # /api/v1/graph/*
│   │   ├── pantheon.rs         # /api/v1/pantheon/*
│   │   ├── auth.rs             # /api/auth/google/*
│   │   ├── health.rs           # /health, /metrics
│   │   └── ...
│   ├── server/                 # 서버 초기화 (8개 파일로 모듈화)
│   ├── acp/                    # ACP/MCP 브릿지 (IDE 통합)
│   ├── middleware/              # 인증, 레이트 리미팅
│   └── websocket/              # /ws/chat, /ws/events
├── migrations/                 # DB 마이그레이션
└── PRD.md                      # 상세 요구사항

~/.cratos/                      # 데이터 디렉토리 (자동 생성)
├── cratos.db                   # SQLite: 이벤트, 실행 기록
├── skills.db                   # SQLite: 스킬, 패턴
├── memory.db                   # SQLite: Graph RAG 대화 메모리
├── vectors/memory/             # HNSW 벡터 인덱스
└── chronicles/                 # 페르소나별 전공 기록
```

## 설정 시스템

설정 우선순위: `.env` > `config/local.toml` > `config/default.toml`

```bash
cratos config list                          # 카테고리별 설정 조회
cratos config get llm.default_provider      # 단일 값 조회
cratos config set llm.default_provider glm  # config/local.toml에 저장
cratos config reset llm.default_provider    # 기본값 복원
cratos config edit                          # $EDITOR로 편집
```

`.env` 변수명 규칙: `CRATOS_LLM__DEFAULT_PROVIDER` (이중 언더스코어 `__`로 중첩 구분)

TUI에서 `F5`로 설정 모달 열기/닫기 가능.

## 슬래시 명령어 (Claude Code)

| 명령어 | 설명 |
|--------|------|
| `/cratos-setup` | 프로젝트 초기 설정 |
| `/develop` | 원격 개발지시 (Issue → PR) |
| `/replay` | 실행 기록 조회/재실행 |

## CLI 명령어

### 기본

| 명령어 | 설명 |
|--------|------|
| `cratos init` | 통합 대화형 설정 마법사 |
| `cratos serve` | 서버 시작 (기본 포트 19527) |
| `cratos doctor` | 시스템 진단 |
| `cratos tui [--persona NAME]` | 대화형 TUI 채팅 |
| `cratos quota [--json] [--watch]` | 프로바이더 할당량/비용 조회 |
| `cratos config {list,get,set,reset,edit}` | 설정 관리 |

### 개발/운영

| 명령어 | 설명 |
|--------|------|
| `cratos develop <issue> [--repo URL] [--dry-run]` | Issue → PR 자동화 |
| `cratos skill {list,show,enable,disable}` | 스킬 관리 |
| `cratos data stats` | 데이터 통계 (레코드 수, 파일 크기) |
| `cratos data clear [sessions\|memory\|history\|chronicles\|vectors\|skills]` | 데이터 삭제 |
| `cratos security audit [--json]` | 보안 감사 |

### 채널/디바이스

| 명령어 | 설명 |
|--------|------|
| `cratos voice [--lang ko]` | 음성 어시스턴트 |
| `cratos browser extension {install,path}` | Chrome 확장 관리 |
| `cratos browser {tabs,open,screenshot}` | 브라우저 제어 |
| `cratos pair {start,devices,unpair}` | 디바이스 페어링 (PIN 기반) |
| `cratos acp [--token T] [--mcp]` | ACP/MCP 브릿지 (IDE 통합) |

### 올림푸스 (페르소나/규칙/기록)

| 명령어 | 설명 |
|--------|------|
| `cratos pantheon {list,show,summon,dismiss}` | 페르소나 관리 |
| `cratos pantheon skill {list,show,claim,release}` | 페르소나-스킬 바인딩 |
| `cratos pantheon skill {leaderboard,summary,sync}` | 스킬 숙련도 조회/동기화 |
| `cratos decrees show {laws,ranks,warfare,alliance,...}` | 율법/규칙 조회 |
| `cratos decrees validate` | 규칙 준수 검증 |
| `cratos chronicle {list,show,log,promote,clean}` | 전공 기록 관리 |

## 올림푸스 OS

그리스/북유럽 신화 기반 3-레이어 에이전트 조직 체계:

| Layer | 이름 | 목적 |
|-------|------|------|
| WHO | **Pantheon** | 에이전트 페르소나 |
| HOW | **Decrees** | 율법, 계급, 개발 규칙 |
| WHAT | **Chronicles** | 전공 기록 및 평가 |

### 페르소나 (14개)

**코어**: Cratos (Orchestrator, Lv255), Athena (PM), Sindri (Dev), Heimdall (QA), Mimir (Researcher)
**확장**: Odin (PO), Hestia (HR), Norns (BA), Apollo (UX), Freya (CS), Tyr (Legal), Nike (Marketing), Thor (DevOps), Brok (Dev)

`@mention` 라우팅: `@athena 이번 스프린트 계획해줘`, `@sindri API 구현해줘`

## REST API

### 핵심 엔드포인트

| Method | Path | 설명 |
|--------|------|------|
| GET | `/health` | 헬스체크 |
| GET | `/health/detailed` | 상세 헬스체크 (DB/Redis 상태) |
| GET | `/metrics` | Prometheus 형식 메트릭 |
| GET/PUT | `/api/v1/config` | 설정 조회/변경 (카테고리별) |
| GET | `/api/v1/tools` | 도구 목록 |
| GET | `/api/v1/quota` | 프로바이더 할당량 |

### 실행/리플레이

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/v1/executions` | 실행 목록 |
| GET | `/api/v1/executions/:id` | 실행 상세 |
| GET | `/api/v1/executions/:id/replay` | 리플레이 이벤트 |
| POST | `/api/v1/executions/:id/rerun` | 재실행 |
| GET | `/api/v1/executions/stats` | 실행 통계 |

### 세션/채팅

| Method | Path | 설명 |
|--------|------|------|
| GET/POST | `/api/v1/sessions` | 세션 목록/생성 |
| POST | `/api/v1/sessions/:id/messages` | 메시지 송신 |
| POST | `/api/v1/sessions/:id/cancel` | 실행 취소 |
| POST | `/api/v1/sessions/init-e2e` | E2E 암호화 초기화 |
| POST | `/api/v1/sessions/decrypt` | 메시지 복호화 |

### 스킬

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/v1/skills` | 스킬 목록 |
| GET/DELETE | `/api/v1/skills/:id` | 스킬 조회/삭제 |
| POST | `/api/v1/skills/:id/enable` | 스킬 활성화 |
| POST | `/api/v1/skills/:id/disable` | 스킬 비활성화 |
| GET | `/api/v1/skills/:id/export` | 스킬 내보내기 |
| POST | `/api/v1/skills/export/bundle` | 번들 내보내기 |
| POST | `/api/v1/skills/import` | 스킬 임포트 |
| GET | `/api/v1/registry/search` | 레지스트리 검색 |
| POST | `/api/v1/registry/install` | 레지스트리에서 설치 |

### 기타

| Method | Path | 설명 |
|--------|------|------|
| GET/POST | `/api/v1/scheduler/tasks` | 스케줄러 작업 관리 |
| GET | `/api/v1/pantheon` | 페르소나 목록 |
| GET | `/api/v1/pantheon/:name` | 페르소나 상세 |
| GET | `/api/v1/graph` | Graph RAG 데이터 |
| GET | `/api/v1/graph/stats` | 그래프 통계 |
| GET/POST/DELETE | `/api/v1/pair/*` | 디바이스 페어링 |
| GET/POST | `/api/v1/browser/*` | 브라우저 제어 |
| GET | `/api/v1/dev/sessions` | 개발 세션 목록 |
| GET | `/api/auth/google/login` | Google OAuth 로그인 |

### WebSocket

| Path | 설명 |
|------|------|
| `/ws/chat` | 대화형 채팅 |
| `/ws/events` | 이벤트 스트림 (실시간 알림) |

## 코딩 규칙

1. `#![forbid(unsafe_code)]` — unsafe 금지
2. `Result<T, E>` 명시적 에러 처리
3. `tracing` 기반 구조화 로깅
4. Conventional Commits 형식
5. 1,000줄 초과 파일 금지 (모듈 분리)

## Gotchas

- **빌드 프로필**: `--release`는 full LTO + codegen-units=1로 ~10분. 일상 테스트는 `--profile dev-release` 사용 (~5분, thin LTO)
- **tui-markdown 버전**: 반드시 `=0.3.5` 사용. `0.3.7`은 ratatui-core 0.1.0 의존 → ratatui 0.29와 타입 불일치
- **환경변수 구분자**: `CRATOS_LLM__DEFAULT_PROVIDER` (이중 언더스코어 `__`). `config-rs`의 `Environment::with_prefix("CRATOS").separator("__")` 사용
- **UTF-8 자르기**: `&text[..max_len]`은 한글 등 멀티바이트 중간에서 패닉. `char_indices()` 사용 필수
- **candle 0.8.x**: 최신 문서와 API 차이 있음. `VarBuilder::from_buffered_safetensors` 사용 (0.9의 `from_safetensors` 없음)
- **DuckDuckGo 웹 검색**: reqwest(rustls)는 JA3 핑거프린팅으로 차단됨. `tokio::process::Command`로 시스템 curl 사용
- **Gemini thought_signature**: Gemini 3 모델은 FunctionCall에 `thoughtSignature` 반환 → 다음 요청에 반드시 보존
- **config/local.toml**: `config-rs`가 `File::with_name("config/local")`로 자동감지하므로 `.json` 파일이 있으면 TOML 설정을 오염시킴

## 참조 문서

- [PRD.md](./PRD.md) — 상세 요구사항
- [.agent/skills/_shared/rust-conventions.md](./.agent/skills/_shared/rust-conventions.md) — Rust 규칙
- [.agent/workflows/develop.md](./.agent/workflows/develop.md) — 개발 워크플로우
- [docs/SETUP_GUIDE.md](./docs/SETUP_GUIDE.md) — 설치 가이드
- [docs/USER_GUIDE.md](./docs/USER_GUIDE.md) — 사용자 가이드
