# Cratos - AI-Powered Personal Assistant

## 프로젝트 개요

Cratos는 Telegram/Slack에서 자연어로 명령을 내리면 AI 에이전트가 이해하고, 정보를 모으고, 실행하고, 결과를 보고하는 **Rust 기반 AI 어시스턴트**입니다.

### 핵심 기능

| 기능 | 설명 |
|------|------|
| **경량 설치** | Docker/PostgreSQL 불필요, SQLite 내장, 단일 바이너리 |
| **자동 스킬 생성** | 사용 패턴 학습 → 자동 스킬 생성 |
| **리플레이 (되감기)** | 타임라인 + 재실행 + dry-run |
| **모델 라우팅** | 작업별 자동 모델 선택, 비용 최적화 |
| **원격 개발 E2E** | Issue → PR 완전 자동화 |
| **Tool Doctor** | 자기 진단 + 해결 체크리스트 |
| **비용/할당량 모니터링** | CostTracker + QuotaTracker, 프로바이더별 사용량 추적 |
| **TUI 채팅** | ratatui 기반 대화형 터미널 인터페이스 |
| **스케줄러** | ProactiveScheduler (Cron/Interval/OneTime) |
| **REST API + WebSocket** | `/api/v1/*` REST 엔드포인트, `/ws/chat`, `/ws/events` 실시간 통신 |

## 기술 스택

- **언어**: Rust 1.88+
- **런타임**: Tokio (비동기)
- **웹**: Axum 0.7
- **DB**: SQLite (sqlx, 내장), Redis (세션용, 선택)
- **채널**: teloxide (Telegram), slack-morphism (Slack), serenity (Discord), matrix-sdk (Matrix)
- **LLM**: 13개 프로바이더 (OpenAI, Anthropic, Gemini, DeepSeek, Groq, Fireworks, SiliconFlow, GLM, Qwen, Moonshot, Novita, OpenRouter, Ollama)
- **임베딩/검색**: tract-onnx (pure Rust ONNX 임베딩), usearch (벡터 인덱스)
- **오디오**: cpal, rodio, hound + candle (로컬 Whisper STT) + Edge TTS (선택적)
- **데이터**: `~/.cratos/cratos.db` (이벤트), `~/.cratos/skills.db` (스킬)

## 프로젝트 구조

```
cratos/
├── .agent/                 # Antigravity 호환 스킬 시스템
│   ├── skills/             # 12개 코어 스킬
│   └── workflows/          # 7개 워크플로우
├── .claude/                # Claude Code 플러그인
│   ├── agents/             # 4개 에이전트
│   ├── commands/           # 3개 슬래시 명령어
│   └── skills/             # 5개 스킬
├── config/
│   ├── default.toml        # 기본 설정
│   ├── pantheon/           # 페르소나 TOML (14개: 5 코어 + 9 확장)
│   └── decrees/            # 율법, 계급, 개발 규칙
├── crates/                 # Rust workspace (11개 크레이트)
│   ├── cratos-core/        # 핵심 오케스트레이션, 보안, 자격증명
│   ├── cratos-channels/    # 채널 어댑터 (Telegram, Slack, Discord, Matrix)
│   ├── cratos-tools/       # 도구 레지스트리, 샌드박스
│   ├── cratos-llm/         # LLM 프로바이더, 임베딩
│   ├── cratos-replay/      # 리플레이 엔진 (SQLite)
│   ├── cratos-skills/      # 자동 스킬 생성 시스템
│   ├── cratos-search/      # 벡터 검색, 시맨틱 인덱싱
│   ├── cratos-memory/      # Graph RAG 대화 메모리
│   ├── cratos-crypto/      # 암호화, 키 관리
│   ├── cratos-audio/       # 음성 제어 (STT/TTS, 선택적)
│   └── cratos-canvas/      # 캔버스 (future)
└── PRD.md                  # 상세 요구사항

~/.cratos/                  # 데이터 디렉토리 (자동 생성)
├── cratos.db               # SQLite: 이벤트, 실행 기록
├── skills.db               # SQLite: 스킬, 패턴
├── memory.db               # SQLite: Graph RAG 대화 메모리
├── vectors/memory/         # HNSW 벡터 인덱스
└── chronicles/             # 페르소나별 전공 기록
```

## 주요 명령어

| 명령어 | 설명 |
|--------|------|
| `/cratos-setup` | 프로젝트 초기 설정 |
| `/develop` | 원격 개발지시 (Issue → PR) |
| `/replay` | 실행 기록 조회/재실행 |

## 올림푸스 OS

그리스/북유럽 신화 기반 3-레이어 에이전트 조직 체계:

| Layer | 이름 | 목적 |
|-------|------|------|
| WHO | **Pantheon** | 에이전트 페르소나 |
| HOW | **Decrees** | 율법, 계급, 개발 규칙 |
| WHAT | **Chronicles** | 전공 기록 및 평가 |

### 페르소나 시스템 (14개)

#### 코어 페르소나

| 역할 | 이름 | 도메인 |
|------|------|--------|
| Orchestrator | Cratos | 전체 통솔 (Lv255) |
| PM | Athena | 전략, 기획 (Lv3) |
| DEV | Sindri | 개발, 구현 (Lv1) |
| QA | Heimdall | 품질, 보안 (Lv2) |
| RESEARCHER | Mimir | 리서치 (Lv4) |

#### 확장 페르소나

| 역할 | 이름 | 도메인 |
|------|------|--------|
| PO | Odin | 프로덕트 오너 (Lv5) |
| HR | Hestia | 인사, 조직 관리 (Lv2) |
| BA | Norns | 비즈니스 분석 (Lv3) |
| UX | Apollo | UX 디자인 (Lv3) |
| CS | Freya | 고객 지원 (Lv2) |
| LEGAL | Tyr | 법무, 규정 (Lv4) |
| MARKETING | Nike | 마케팅 (Lv2) |
| DEVOPS | Thor | 인프라, 운영 (Lv3) |
| DEV | Brok | 개발 (Lv1) |

### @mention 라우팅

```
@athena 이번 스프린트 계획해줘     # PM
@sindri API 구현해줘              # DEV
@heimdall 보안 리뷰해줘           # QA
@mimir 이 기술 조사해줘           # RESEARCHER
@cratos 상황 정리해줘             # Orchestrator
```

### CLI 명령어

| 명령어 | 설명 |
|--------|------|
| `cratos init` | 통합 대화형 설정 마법사 |
| `cratos serve` | 서버 시작 |
| `cratos doctor` | 시스템 진단 |
| `cratos quota` | 프로바이더 할당량/비용 조회 |
| `cratos tui` | 대화형 TUI 채팅 |
| `cratos pantheon list` | 페르소나 목록 |
| `cratos pantheon show <name>` | 페르소나 상세 |
| `cratos pantheon summon <name>` | 페르소나 소환 (활성화) |
| `cratos pantheon dismiss` | 활성 페르소나 해제 |
| `cratos decrees show laws` | 율법 보기 |
| `cratos decrees show ranks` | 계급 체계 |
| `cratos decrees show warfare` | 개발 규칙 |
| `cratos decrees show alliance` | 협업 규칙 |
| `cratos decrees show tribute` | 보상/비용 규칙 |
| `cratos decrees show judgment` | 평가 프레임워크 |
| `cratos decrees show culture` | 문화/가치관 |
| `cratos decrees show operations` | 운영 절차 |
| `cratos decrees validate` | 규칙 검증 |
| `cratos chronicle list` | 전공 기록 목록 |
| `cratos chronicle show <name>` | 개별 기록 |
| `cratos chronicle log "msg"` | 기록 추가 |
| `cratos chronicle promote <name>` | 승급 요청 |

## 스킬 목록

| 스킬 | 역할 |
|------|------|
| rust-agent | Rust 백엔드 개발 |
| channel-agent | Telegram/Slack 연동 |
| llm-agent | LLM 프로바이더 연동 |
| replay-agent | 리플레이 엔진 (핵심!) |
| debug-agent | Tool Doctor 진단 |
| qa-agent | 테스트/보안 검증 |
| infra-agent | Docker/K8s/CI |
| docs-agent | 문서 자동 생성 |
| pm-agent | 계획 수립 |
| commit | Git 커밋/PR |
| orchestrator | 멀티-에이전트 실행 |
| workflow-guide | 워크플로우 가이드 |

## REST API & WebSocket

### REST 엔드포인트 (`/api/v1/*`)

| Method | Path | 설명 |
|--------|------|------|
| GET | `/health` | 헬스체크 (간단) |
| GET | `/health/detailed` | 상세 헬스체크 (DB/Redis 상태) |
| GET | `/metrics` | Prometheus 형식 메트릭 |
| GET/PUT | `/api/v1/config` | 설정 조회/변경 |
| GET | `/api/v1/tools` | 도구 목록 조회 |
| GET | `/api/v1/executions` | 실행 목록 조회 |
| GET | `/api/v1/executions/{id}` | 실행 상세 조회 |
| GET | `/api/v1/executions/{id}/replay` | 리플레이 이벤트 조회 |
| POST | `/api/v1/executions/{id}/rerun` | 재실행 |
| GET/POST/PUT/DELETE | `/api/v1/scheduler/tasks` | 스케줄러 작업 관리 |
| GET | `/api/v1/quota` | 프로바이더 할당량 조회 |

### WebSocket 엔드포인트

| Path | 설명 |
|------|------|
| `/ws/chat` | 대화형 채팅 |
| `/ws/events` | 이벤트 스트림 (실시간 알림) |

## 코딩 규칙

1. `#![forbid(unsafe_code)]` - unsafe 금지
2. `Result<T, E>` 명시적 에러 처리
3. `tracing` 기반 구조화 로깅
4. 테스트 커버리지 70% 이상
5. Conventional Commits 형식

## 참조 문서

- [PRD.md](./PRD.md) - 상세 요구사항
- [.agent/skills/_shared/rust-conventions.md](./.agent/skills/_shared/rust-conventions.md) - Rust 규칙
- [.agent/workflows/develop.md](./.agent/workflows/develop.md) - 개발 워크플로우
