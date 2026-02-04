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

## 기술 스택

- **언어**: Rust 1.80+
- **런타임**: Tokio (비동기)
- **웹**: Axum 0.7
- **DB**: SQLite (sqlx, 내장), Redis (세션용, 선택)
- **채널**: teloxide (Telegram), slack-morphism (Slack), serenity (Discord), matrix-sdk (Matrix)
- **LLM**: async-openai (OpenAI), reqwest (Anthropic), tiktoken-rs (토큰 카운팅)
- **임베딩/검색**: fastembed (로컬 임베딩), usearch (벡터 인덱스)
- **오디오**: cpal, rodio (선택적)
- **데이터**: `~/.cratos/cratos.db` (이벤트), `~/.cratos/skills.db` (스킬)

## 프로젝트 구조

```
cratos/
├── .agent/                 # Antigravity 호환 스킬 시스템
│   ├── skills/             # 11개 코어 스킬
│   └── workflows/          # 7개 워크플로우
├── .claude/                # Claude Code 플러그인
│   ├── agents/             # 4개 에이전트
│   ├── commands/           # 3개 슬래시 명령어
│   └── skills/             # 5개 스킬
├── crates/                 # Rust workspace
│   ├── cratos-core/        # 핵심 오케스트레이션, 보안, 자격증명
│   ├── cratos-channels/    # 채널 어댑터 (Telegram, Slack, Discord, Matrix)
│   ├── cratos-tools/       # 도구 레지스트리, 샌드박스
│   ├── cratos-llm/         # LLM 프로바이더, 임베딩
│   ├── cratos-replay/      # 리플레이 엔진 (SQLite)
│   ├── cratos-skills/      # 자동 스킬 생성 시스템 ⭐
│   ├── cratos-search/      # 벡터 검색, 시맨틱 인덱싱
│   ├── cratos-audio/       # 음성 제어 (STT/TTS, 선택적)
│   └── cratos-canvas/      # 캔버스 (future)
└── PRD.md                  # 상세 요구사항

~/.cratos/                  # 데이터 디렉토리 (자동 생성)
├── cratos.db               # SQLite: 이벤트, 실행 기록
└── skills.db               # SQLite: 스킬, 패턴
```

## 주요 명령어

| 명령어 | 설명 |
|--------|------|
| `/cratos-setup` | 프로젝트 초기 설정 |
| `/develop` | 원격 개발지시 (Issue → PR) |
| `/replay` | 실행 기록 조회/재실행 |

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
