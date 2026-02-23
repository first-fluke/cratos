# Cratos - AI-Powered Personal Assistant

Telegram/Slack에서 자연어로 명령을 내리면 AI 에이전트가 이해하고, 정보를 모으고, 실행하고, 결과를 보고하는 **Rust 기반 AI 어시스턴트**입니다.

## 원클릭 설치

### macOS / Linux
```bash
curl -sSL https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.sh | sh
```

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.ps1 | iex
```

설치 스크립트가 자동으로:
- 플랫폼에 맞는 바이너리 다운로드
- PATH에 설치
- 한국어 설정 마법사 실행

## 주요 기능

- **경량 설치**: 내장 SQLite로 설치 즉시 실행 (`~/.cratos/cratos.db`)
- **자동 스킬 생성**: 사용 패턴을 학습하여 자동으로 워크플로우 스킬 생성
- **멀티 LLM 지원**: OpenAI, Anthropic, Gemini, DeepSeek, Groq, Fireworks, SiliconFlow, GLM, Qwen, Moonshot, Novita, OpenRouter, Ollama (13개 프로바이더)
- **스마트 라우팅**: 작업 유형별 자동 모델 선택으로 비용 70% 절감
- **무료 모델 지원**: Z.AI GLM (완전 무료, 일일 제한 없음), Gemini Flash, Groq, Novita, SiliconFlow, Ollama
- **리플레이 엔진**: 모든 실행 기록을 이벤트로 저장, 타임라인 조회 및 재실행
- **도구 시스템**: 파일, HTTP, Git, GitHub, 명령 실행, PTY bash, 브라우저, 웹 검색, 에이전트 CLI, WoL, 설정 등 20개 빌트인 도구 + MCP 확장
- **채널 어댑터**: Telegram, Slack, Discord, Matrix, WhatsApp — 슬래시 명령어, DM 정책, EventBus 알림
- **Chrome Extension**: WebSocket 게이트웨이를 통한 브라우저 원격 제어
- **Graph RAG 메모리**: 엔티티 그래프 + 하이브리드 벡터 검색으로 세션 간 대화 기억
- **TUI 채팅**: ratatui 기반 대화형 터미널 (마크다운 렌더링, 마우스 스크롤, 쿼터 표시)
- **웹 검색**: DuckDuckGo 기반 내장 검색 (API 키 불필요)
- **MCP 통합**: `.mcp.json`에서 MCP 서버 자동 탐지, SSE/stdio 지원
- **보안 강화**: Docker 샌드박스, 자격증명 암호화, 프롬프트 인젝션 방어
- **올림푸스 OS**: 신화 기반 3-레이어 에이전트 조직 체계 (Pantheon/Decrees/Chronicles)

## 시스템 요구사항

| 항목 | 최저 사양¹ | 최소 사양 | 권장 사양 |
|------|-----------|----------|----------|
| **OS** | macOS 11+, Windows 10, Ubuntu 20.04+ | macOS 12+, Windows 10+, Ubuntu 22.04+ | 최신 버전 |
| **CPU** | 1코어 | 1코어 | 2코어 이상 |
| **RAM** | 256MB (실행) / 2GB (빌드) | 1GB (실행) / 4GB (빌드) | 4GB 이상 |
| **디스크** | 100MB | 1GB | 5GB 이상 |
| **Rust** | 1.88+ | 1.88+ | 최신 stable |
| **Docker** | - | 선택사항 | 최신 버전 |

> ¹ **최저 사양**: 임베딩 비활성화 시 (`cargo build --no-default-features`). 시맨틱 검색 불가.
>
> **참고**: PostgreSQL, Docker 없이 실행 가능합니다. 데이터는 `~/.cratos/cratos.db` (SQLite)에 저장됩니다.

### Ollama 로컬 LLM 사용 시 (추가)

| 모델 | RAM | VRAM (GPU) |
|------|-----|------------|
| Llama 3.2 3B | 4GB | 4GB |
| Llama 3.2 7B | 8GB | 8GB |
| Llama 3.1 70B | 48GB | 48GB |

> **참고**: 외부 LLM API(OpenAI, Anthropic 등) 사용 시 GPU 불필요

## 빠른 시작

### 방법 1: 원클릭 설치 (권장)

```bash
# macOS / Linux
curl -sSL https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.ps1 | iex
```

설정 마법사가 안내하는 대로 따라하세요:
1. Telegram 봇 만들기 (링크 제공)
2. AI 프로바이더 선택 (무료 옵션 제공)
3. 연결 테스트

### 방법 2: 수동 설정

```bash
# 저장소 클론
git clone https://github.com/first-fluke/cratos.git
cd cratos

# 설정 마법사 실행 (한국어)
cargo run -- init --lang ko

# 영어 설정
cargo run -- init
```

### 방법 3: 소스에서 빌드

```bash
# 환경 변수 파일 생성
cp .env.example .env

# .env 파일 수정 (API 키 입력)
vim .env

# 빌드 및 실행
cargo build --release
cargo run --release

# 헬스체크
curl http://localhost:19527/health
```

데이터는 자동으로 `~/.cratos/cratos.db`에 저장됩니다.

### 설정

| 명령어 | 설명 |
|--------|------|
| `cratos init` | 통합 대화형 설정 마법사 (언어 자동 감지) |
| `cratos init --lang ko` | 한국어 설정 마법사 |

### Docker로 실행 (선택)

```bash
# Redis만 실행 (세션 저장용, 선택사항)
docker-compose up -d redis

# Cratos 실행
cargo run
```

## 프로젝트 구조

```
cratos/
├── crates/
│   ├── cratos-core/      # 오케스트레이션 엔진, 보안, 자격증명
│   ├── cratos-channels/  # 채널 어댑터 (Telegram, Slack, Discord, Matrix)
│   ├── cratos-tools/     # 도구 레지스트리, 샌드박스
│   ├── cratos-llm/       # LLM 프로바이더, 토큰 카운팅, ONNX 임베딩, 쿼터 추적
│   ├── cratos-replay/    # 이벤트 로깅 및 리플레이 (SQLite)
│   ├── cratos-skills/    # 자동 스킬 생성 시스템
│   ├── cratos-search/    # 벡터 검색 (usearch), 시맨틱 인덱싱
│   ├── cratos-memory/    # Graph RAG 대화 메모리 (엔티티 그래프 + 하이브리드 검색)
│   ├── cratos-crypto/    # 암호화 유틸리티
│   ├── cratos-audio/     # 음성 제어 (STT/TTS, 선택적)
│   └── cratos-canvas/    # Live Canvas (future)
├── config/
│   ├── default.toml      # 기본 설정
│   ├── pantheon/         # 페르소나 TOML 파일 (14개: 5 코어 + 9 확장)
│   └── decrees/          # 율법, 계급, 개발 규칙
├── src/
│   ├── main.rs           # 애플리케이션 진입점
│   ├── cli/              # CLI 명령어 (init, doctor, quota, tui, pantheon, decrees, chronicle)
│   ├── api/              # REST API (config, tools, executions, scheduler, quota)
│   ├── websocket/        # WebSocket 핸들러 (chat, events)
│   └── server.rs         # 서버 초기화

~/.cratos/                # 데이터 디렉토리 (자동 생성)
├── cratos.db             # SQLite 메인 DB (이벤트, 실행 기록)
├── skills.db             # SQLite 스킬 DB (스킬, 패턴)
└── chronicles/           # 페르소나별 전공 기록
```

## 설정

### 환경 변수

| 변수 | 설명 | 필수 |
|------|------|------|
| `REDIS_URL` | Redis 연결 URL (세션용, 없으면 메모리 사용) | |
| `TELEGRAM_BOT_TOKEN` | Telegram 봇 토큰 | △ |
| `SLACK_BOT_TOKEN` | Slack 봇 토큰 | △ |
| **LLM API 키 (하나 이상)** | | |
| `OPENAI_API_KEY` | OpenAI API 키 | |
| `ANTHROPIC_API_KEY` | Anthropic API 키 | |
| `GEMINI_API_KEY` | Google Gemini API 키 (권장) | |
| `GOOGLE_API_KEY` | Google Gemini API 키 (별칭) | |
| `ZHIPU_API_KEY` | Z.AI GLM API 키 (Flash 모델 무료) | |
| `DASHSCOPE_API_KEY` | Alibaba Qwen API 키 | |
| `OPENROUTER_API_KEY` | OpenRouter API 키 | |
| `NOVITA_API_KEY` | Novita AI API 키 (무료) | |
| `ELEVENLABS_API_KEY` | ElevenLabs TTS API 키 (선택) | |
| **설정 오버라이드** | | |
| `CRATOS_LLM__DEFAULT_PROVIDER` | 기본 LLM 프로바이더 설정 | |

> **참고**: `DATABASE_URL`은 더 이상 필요 없습니다. 내장 SQLite를 사용합니다.

### 설정 파일

`config/default.toml`에서 기본 설정을 확인하고, `config/local.toml`을 생성하여 로컬 환경에 맞게 커스터마이즈할 수 있습니다.

## LLM 프로바이더

### 유료 프로바이더

| 프로바이더 | 모델 | 특징 |
|-----------|------|------|
| **OpenAI** | GPT-5, GPT-5.2, GPT-5-nano | 최신 세대, 코딩 |
| **Anthropic** | Claude Sonnet 4.5, Claude Haiku 4.5, Claude Opus 4.5 | 코드 생성 우수 |
| **Gemini** | Gemini 3 Pro, Gemini 3 Flash, Gemini 2.5 Pro | 긴 컨텍스트, 멀티모달, Standard API only (안전) |
| **GLM** | GLM-4.7, GLM-4.7-Flash (무료), GLM-5 | ZhipuAI 모델 |
| **Qwen** | Qwen3-Max, Qwen3-Plus, Qwen3-Flash, Qwen3-Coder | 다국어, 코딩, 추론 |
| **DeepSeek** | DeepSeek-V3.2, DeepSeek-R1 | 초저가, 추론 |

### 무료/저가 프로바이더

| 프로바이더 | 모델 | 제한 |
|-----------|------|------|
| **Z.AI (GLM)** | GLM-4.7-Flash, GLM-4.5-Flash | 완전 무료, 일일 제한 없음 |
| **Gemini** | Gemini 2.0 Flash | 무료 (일 1,500회) |
| **Groq** | Llama 3.1 8B, GPT-OSS 20B | 무료 가능 |
| **Novita** | Qwen2.5-7B, GLM-4-9B | 무료 가입 |
| **SiliconFlow** | Qwen2.5-7B | 무료 모델 제공 |
| **Ollama** | 모든 로컬 모델 | 무제한 (로컬) |

### 모델 라우팅

작업 유형에 따라 자동으로 적절한 모델을 선택합니다:

| 작업 유형 | 모델 티어 | 예시 모델 |
|----------|-----------|-----------|
| Classification | Fast | GPT-5-nano, Claude Haiku 4.5 |
| Summarization | Fast | GPT-5-nano, Gemini 2.0 Flash |
| Conversation | Standard | GPT-5, Claude Sonnet 4.5 |
| CodeGeneration | Standard | GPT-5, Claude Sonnet 4.5 |
| Planning | Premium | GPT-5.2, Claude Opus 4.5 |

## 올림푸스 OS (에이전트 조직 체계)

신화 기반 3-레이어 에이전트 조직 시스템:

| Layer | 이름 | 목적 |
|-------|------|------|
| WHO | **Pantheon** | 14개 에이전트 페르소나 (5 코어 + 9 확장) |
| HOW | **Decrees** | 율법, 계급, 개발 규칙 |
| WHAT | **Chronicles** | 전공 기록 및 평가 |

### 코어 페르소나

| 역할 | 이름 | 도메인 |
|------|------|--------|
| Orchestrator | **Cratos** | 최고 통솔자 (Lv255) |
| PM | **Athena** | 전략, 기획 (Lv3) |
| DEV | **Sindri** | 개발, 구현 (Lv1) |
| QA | **Heimdall** | 품질, 보안 (Lv2) |
| RESEARCHER | **Mimir** | 리서치, 분석 (Lv4) |

### 확장 페르소나

| 역할 | 이름 | 도메인 |
|------|------|--------|
| PO | **Odin** | 프로덕트 오너 (Lv5) |
| HR | **Hestia** | 인사, 조직 관리 (Lv2) |
| BA | **Norns** | 비즈니스 분석 (Lv3) |
| UX | **Apollo** | UX 디자인 (Lv3) |
| CS | **Freya** | 고객 지원 (Lv2) |
| LEGAL | **Tyr** | 법무, 규정 (Lv4) |
| MARKETING | **Nike** | 마케팅 (Lv2) |
| DEVOPS | **Thor** | 인프라, 운영 (Lv3) |
| DEV | **Brok** | 개발 (Lv1) |

### @mention 라우팅

@mention으로 특정 페르소나에게 작업 지시:

```
@athena 이번 스프린트 계획해줘     # PM - 전략
@sindri API 구현해줘              # DEV - 개발
@heimdall 보안 리뷰해줘           # QA - 품질
@mimir 이 기술 조사해줘           # RESEARCHER - 분석
@cratos 상황 정리해줘             # Orchestrator
```

응답 형식: `[Persona LvN] 율법 제N조에 의거하여...`

### CLI 명령어

```bash
# 설정
cratos init                       # 통합 대화형 설정 마법사 (언어 자동 감지)
cratos init --lang ko             # 한국어 설정 마법사

# 시스템
cratos serve                      # 서버 시작
cratos doctor                     # 진단 실행
cratos quota                      # 프로바이더 할당량/비용 조회
cratos tui                        # 대화형 TUI 채팅

# Pantheon (페르소나)
cratos pantheon list              # 페르소나 목록
cratos pantheon show sindri       # 페르소나 상세 보기
cratos pantheon summon sindri     # 페르소나 소환 (활성화)
cratos pantheon dismiss           # 활성 페르소나 해제

# Decrees (규약)
cratos decrees show laws          # 율법 보기
cratos decrees show ranks         # 계급 체계 보기
cratos decrees show warfare       # 개발 규칙
cratos decrees show alliance      # 협업 규칙
cratos decrees show tribute       # 보상/비용 규칙
cratos decrees show judgment      # 평가 프레임워크
cratos decrees show culture       # 문화/가치관
cratos decrees show operations    # 운영 절차
cratos decrees validate           # 규칙 준수 검증

# Chronicles (전공 기록)
cratos chronicle list             # 전공 기록 목록
cratos chronicle show sindri      # 개별 기록 보기
cratos chronicle log "메시지"     # 기록 추가
cratos chronicle promote sindri   # 승급 요청
```

## 보안 기능

> **Security-first by design** — 보안을 사후 대응이 아닌, 설계 단계부터 핵심 원칙으로 구축했습니다.

- **메모리 안전 기반**: Rust로 작성, `#![forbid(unsafe_code)]` — 버퍼 오버플로우, use-after-free 불가능
- **평문 비밀 제로**: 모든 자격증명은 OS 키체인으로 암호화 (Keychain, Secret Service, Credential Manager)
- **기본 격리 활성화**: 샌드박스가 기본 활성화되고 네트워크 차단됨, 옵트인 아님
- **내장 위협 탐지**: 20+ 프롬프트 인젝션 패턴을 자동 탐지 및 차단
- **도구별 위험도 분류**: 모든 도구에 명시적 위험 수준과 적절한 보호장치 적용
- **입출력 검증**: 모든 사용자 입력과 LLM 출력을 실행 전 검증

### Docker 샌드박스

위험한 도구는 Docker 컨테이너에서 격리 실행됩니다:

```toml
[security.sandbox]
default_network = "none"  # 네트워크 차단
max_memory_mb = 512       # 메모리 제한
max_cpu_percent = 50      # CPU 제한
```

### 자격증명 암호화

API 키를 OS 키체인에 안전하게 저장합니다:
- macOS: Keychain
- Linux: Secret Service (GNOME Keyring)
- Windows: Credential Manager

### 프롬프트 인젝션 방어

악성 프롬프트를 자동 탐지하고 차단합니다:
- 20+ 위험 패턴 탐지
- 입력/출력 검증
- 민감 정보 노출 방지

## 지원 도구

| 도구 | 설명 | 위험도 |
|------|------|--------|
| `file_read` | 파일 읽기 | Low |
| `file_write` | 파일 쓰기 | Medium |
| `file_list` | 디렉토리 목록 | Low |
| `http_get` | HTTP GET 요청 | Low |
| `http_post` | HTTP POST 요청 | Medium |
| `exec` | 명령 실행 (샌드박스/Docker) | High |
| `bash` | Bash 쉘 실행 (exit code 1 허용) | High |
| `git_status` | Git 상태 조회 | Low |
| `git_commit` | Git 커밋 생성 | Medium |
| `git_branch` | Git 브랜치 관리 | Medium |
| `git_diff` | Git diff 조회 | Low |
| `git_push` | Git 원격 푸시 | High |
| `git_clone` | Git 저장소 클론 | Medium |
| `git_log` | Git 커밋 로그 조회 | Low |
| `github_api` | GitHub API 연동 | Medium |
| `browser` | 브라우저 자동화 (MCP) | Medium |
| `wol` | Wake-on-LAN | Medium |
| `config` | 자연어 설정 변경 | Medium |
| `web_search` | DuckDuckGo 웹 검색 (API 키 불필요) | Low |
| `agent_cli` | 외부 AI 에이전트 CLI 실행 | High |

## 테스트

```bash
# 전체 테스트 실행
cargo test --workspace

# 통합 테스트만 실행
cargo test --test integration_test

# 특정 크레이트 테스트
cargo test -p cratos-llm
cargo test -p cratos-tools
cargo test -p cratos-core
```

## 문서

### 기본 가이드
- [설치 가이드](./docs/SETUP_GUIDE.md) | [English](./docs/en/SETUP_GUIDE.md)
- [사용 가이드](./docs/USER_GUIDE.md) | [English](./docs/en/USER_GUIDE.md)
- [개발자 테스트 가이드](./docs/TEST_GUIDE_DEV.md) | [English](./docs/en/TEST_GUIDE_DEV.md)
- [사용자 테스트 가이드](./docs/TEST_GUIDE_USER.md) | [English](./docs/en/TEST_GUIDE_USER.md)

### 채널 연동 가이드

| 가이드 | 설명 |
|--------|------|
| [Telegram](./docs/guides/TELEGRAM.md) \| [EN](./docs/en/guides/TELEGRAM.md) | 텔레그램 봇 (슬래시 명령어, DM 정책) |
| [Slack](./docs/guides/SLACK.md) \| [EN](./docs/en/guides/SLACK.md) | Slack 앱 (Socket Mode) |
| [Discord](./docs/guides/DISCORD.md) \| [EN](./docs/en/guides/DISCORD.md) | Discord 봇 (serenity) |
| [WhatsApp](./docs/guides/WHATSAPP.md) \| [EN](./docs/en/guides/WHATSAPP.md) | WhatsApp (Baileys / Business API) |

### 기능 가이드

| 가이드 | 설명 |
|--------|------|
| [브라우저 자동화](./docs/guides/BROWSER_AUTOMATION.md) \| [EN](./docs/en/guides/BROWSER_AUTOMATION.md) | MCP 기반 브라우저 제어 |
| [스킬 자동 생성](./docs/guides/SKILL_AUTO_GENERATION.md) \| [EN](./docs/en/guides/SKILL_AUTO_GENERATION.md) | 패턴 학습 → 스킬 변환 |
| [Graceful Shutdown](./docs/guides/GRACEFUL_SHUTDOWN.md) \| [EN](./docs/en/guides/GRACEFUL_SHUTDOWN.md) | 5-Phase 안전 종료 |
| [Live Canvas](./docs/guides/LIVE_CANVAS.md) \| [EN](./docs/en/guides/LIVE_CANVAS.md) | 실시간 시각적 워크스페이스 |
| [Native Apps](./docs/guides/NATIVE_APPS.md) \| [EN](./docs/en/guides/NATIVE_APPS.md) | Tauri 데스크톱 앱 |

## 라이선스

MIT

## 기여

이슈와 PR을 환영합니다.
