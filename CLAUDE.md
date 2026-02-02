# Cratos - AI-Powered Personal Assistant

## 프로젝트 개요

Cratos는 Telegram/Slack에서 자연어로 명령을 내리면 AI 에이전트가 이해하고, 정보를 모으고, 실행하고, 결과를 보고하는 **Rust 기반 AI 어시스턴트**입니다.

### 핵심 차별화 (vs OpenClaw)

| 기능 | OpenClaw | Cratos |
|------|----------|--------|
| **리플레이 (되감기)** | ❌ | ✅ 타임라인 + 재실행 + dry-run |
| **모델 라우팅** | ❌ | ✅ 작업별 자동 선택, 70% 비용 절감 |
| **원격 개발 E2E** | △ | ✅ Issue → PR 완전 자동화 |
| **Tool Doctor** | ❌ | ✅ 자기 진단 + 해결 체크리스트 |

## 기술 스택

- **언어**: Rust 1.75+
- **런타임**: Tokio (비동기)
- **웹**: Axum 0.7
- **DB**: PostgreSQL 16 (sqlx), Redis 7
- **채널**: teloxide (Telegram), slack-morphism (Slack)
- **LLM**: async-openai (OpenAI), reqwest (Anthropic)

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
├── crates/                 # Rust workspace (구현 예정)
│   ├── cratos-core/        # 핵심 오케스트레이션
│   ├── cratos-channels/    # 채널 어댑터
│   ├── cratos-tools/       # 도구 레지스트리
│   ├── cratos-llm/         # LLM 프로바이더
│   └── cratos-replay/      # 리플레이 엔진
└── PRD.md                  # 상세 요구사항
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
