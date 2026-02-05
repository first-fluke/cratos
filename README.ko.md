# Cratos - AI-Powered Personal Assistant

Telegram/Slack에서 자연어로 명령을 내리면 AI 에이전트가 이해하고, 정보를 모으고, 실행하고, 결과를 보고하는 **Rust 기반 AI 어시스턴트**입니다.

## 주요 기능

- **경량 설치**: 내장 SQLite로 설치 즉시 실행 (`~/.cratos/cratos.db`)
- **자동 스킬 생성**: 사용 패턴을 학습하여 자동으로 워크플로우 스킬 생성
- **멀티 LLM 지원**: OpenAI, Anthropic, Gemini, Ollama, GLM, Qwen, OpenRouter, Novita, Groq, DeepSeek
- **스마트 라우팅**: 작업 유형별 자동 모델 선택으로 비용 70% 절감
- **무료 모델 지원**: OpenRouter, Novita를 통한 무료 LLM 사용 (Llama, Qwen, GLM)
- **리플레이 엔진**: 모든 실행 기록을 이벤트로 저장, 타임라인 조회 및 재실행
- **도구 시스템**: 파일, HTTP, Git, GitHub, 명령 실행 등 11개 빌트인 도구
- **채널 어댑터**: Telegram, Slack, Discord, Matrix 지원
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

### 1. 환경 설정

```bash
# 환경 변수 파일 생성
cp .env.example .env

# .env 파일 수정 (API 키 입력)
vim .env
```

### 2. 실행

```bash
# 빌드 및 실행
cargo build --release
cargo run --release

# 또는 한 번에
cargo run

# 헬스체크
curl http://localhost:9742/health
```

데이터는 자동으로 `~/.cratos/cratos.db`에 저장됩니다.

### 3. Docker로 실행 (선택)

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
│   ├── cratos-llm/       # LLM 프로바이더, 토큰 카운팅, 임베딩
│   ├── cratos-replay/    # 이벤트 로깅 및 리플레이 (SQLite)
│   ├── cratos-skills/    # 자동 스킬 생성 시스템
│   ├── cratos-search/    # 벡터 검색, 시맨틱 인덱싱
│   ├── cratos-audio/     # 음성 제어 (STT/TTS, 선택적)
│   └── cratos-canvas/    # 캔버스 (future)
├── config/
│   ├── default.toml      # 기본 설정
│   ├── pantheon/         # 페르소나 TOML 파일 (5개 코어 페르소나)
│   └── decrees/          # 율법, 계급, 개발 규칙
└── src/main.rs           # 애플리케이션 진입점

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
| `GOOGLE_API_KEY` | Google Gemini API 키 | |
| `ZHIPU_API_KEY` | ZhipuAI GLM API 키 | |
| `DASHSCOPE_API_KEY` | Alibaba Qwen API 키 | |
| `OPENROUTER_API_KEY` | OpenRouter API 키 | |
| `NOVITA_API_KEY` | Novita AI API 키 (무료) | |

> **참고**: `DATABASE_URL`은 더 이상 필요 없습니다. 내장 SQLite를 사용합니다.

### 설정 파일

`config/default.toml`에서 기본 설정을 확인하고, `config/local.toml`을 생성하여 로컬 환경에 맞게 커스터마이즈할 수 있습니다.

## LLM 프로바이더

### 유료 프로바이더

| 프로바이더 | 모델 | 특징 |
|-----------|------|------|
| **OpenAI** | GPT-5.2, GPT-5.2-mini | 범용, 도구 호출 우수 |
| **Anthropic** | Claude 3.5 Sonnet/Haiku | 코드 생성 우수 |
| **Gemini** | Gemini 1.5 Pro/Flash | 긴 컨텍스트 |
| **GLM** | GLM-4-9B, GLM-Z1-9B | 중국어 특화 |
| **Qwen** | Qwen-Turbo/Plus/Max | 다국어, 코딩 |
| **DeepSeek** | DeepSeek-V3, DeepSeek-R1 | 초저가 ($0.14/1M 토큰) |

### 무료/저가 프로바이더

| 프로바이더 | 모델 | 제한 |
|-----------|------|------|
| **OpenRouter** | Qwen3-32B, Llama 3.2, Gemma 2 | 1000회/일 |
| **Novita** | Qwen2.5-7B, GLM-4-9B, Llama 3.2 | 무료 가입 |
| **Groq** | Llama 3.3 70B, Mixtral 8x7B | 무료, 초고속 추론 |
| **Ollama** | 모든 로컬 모델 | 무제한 (하드웨어 의존) |

### 모델 라우팅

작업 유형에 따라 자동으로 적절한 모델을 선택합니다:

| 작업 유형 | 모델 티어 | 예시 모델 |
|----------|-----------|-----------|
| Classification | Fast | GPT-5.2-mini, Claude Haiku |
| Summarization | Fast | GPT-5.2-mini, Gemini Flash |
| Conversation | Standard | GPT-5.2, Claude Sonnet |
| CodeGeneration | Standard | GPT-5.2, Claude Sonnet |
| Planning | Premium | GPT-5.2-turbo, Claude Opus |

## 올림푸스 OS (에이전트 조직 체계)

신화 기반 3-레이어 에이전트 조직 시스템:

| Layer | 이름 | 목적 |
|-------|------|------|
| WHO | **Pantheon** | 에이전트 페르소나 (Cratos, Athena, Sindri, Heimdall, Mimir) |
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
# Pantheon (페르소나)
cratos pantheon list              # 페르소나 목록
cratos pantheon show sindri       # 페르소나 상세 보기

# Decrees (규약)
cratos decrees show laws          # 율법 보기
cratos decrees show ranks         # 계급 체계 보기
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
| `exec` | 명령 실행 (샌드박스) | High |
| `git_status` | Git 상태 조회 | Low |
| `git_commit` | Git 커밋 생성 | Medium |
| `git_branch` | Git 브랜치 관리 | Medium |
| `git_diff` | Git diff 조회 | Low |
| `github_api` | GitHub API 연동 | Medium |

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

- [설치 가이드](./docs/SETUP_GUIDE.md) - 처음 설치하는 분
- [사용 가이드](./docs/USER_GUIDE.md) - 기능 사용법
- [PRD](./PRD.md) - 상세 요구사항

## 라이선스

MIT

## 기여

이슈와 PR을 환영합니다.
