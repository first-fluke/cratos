# Cratos - AI-Powered Personal Assistant

Telegram/Slack에서 자연어로 명령을 내리면 AI 에이전트가 이해하고, 정보를 모으고, 실행하고, 결과를 보고하는 **Rust 기반 AI 어시스턴트**입니다.

## 주요 기능

- **멀티 LLM 지원**: OpenAI, Anthropic, Gemini, Ollama
- **스마트 라우팅**: 작업 유형별 자동 모델 선택으로 비용 70% 절감
- **리플레이 엔진**: 모든 실행 기록을 이벤트로 저장, 타임라인 조회 및 재실행
- **도구 시스템**: 파일, HTTP, Git, GitHub, 명령 실행 등 11개 빌트인 도구
- **채널 어댑터**: Telegram, Slack 지원

## 빠른 시작

### 1. 환경 설정

```bash
# 환경 변수 파일 생성
cp .env.example .env

# .env 파일 수정 (API 키 입력)
vim .env
```

### 2. Docker로 실행

```bash
# 전체 스택 실행 (PostgreSQL, Redis, Cratos)
docker-compose up -d

# 로그 확인
docker-compose logs -f cratos

# 헬스체크
curl http://localhost:9742/health
```

### 3. 로컬 개발

```bash
# 의존성 설치
cargo build

# PostgreSQL & Redis 실행
docker-compose up -d postgres redis

# 마이그레이션 실행
sqlx migrate run

# 서버 실행
cargo run
```

## 프로젝트 구조

```
cratos/
├── crates/
│   ├── cratos-core/      # 오케스트레이션 엔진
│   ├── cratos-channels/  # 채널 어댑터 (Telegram, Slack)
│   ├── cratos-tools/     # 도구 레지스트리 및 빌트인
│   ├── cratos-llm/       # LLM 프로바이더 추상화
│   └── cratos-replay/    # 이벤트 로깅 및 리플레이
├── config/               # 설정 파일
├── migrations/           # 데이터베이스 마이그레이션
└── src/main.rs           # 애플리케이션 진입점
```

## 설정

### 환경 변수

| 변수 | 설명 |
|------|------|
| `DATABASE_URL` | PostgreSQL 연결 URL |
| `REDIS_URL` | Redis 연결 URL |
| `OPENAI_API_KEY` | OpenAI API 키 |
| `ANTHROPIC_API_KEY` | Anthropic API 키 |
| `TELEGRAM_BOT_TOKEN` | Telegram 봇 토큰 |
| `SLACK_BOT_TOKEN` | Slack 봇 토큰 |
| `SLACK_SIGNING_SECRET` | Slack 서명 시크릿 |

### 설정 파일

`config/default.toml`에서 기본 설정을 확인하고, `config/local.toml`을 생성하여 로컬 환경에 맞게 커스터마이즈할 수 있습니다.

## 지원 도구

| 도구 | 설명 | 위험도 |
|------|------|--------|
| `file_read` | 파일 읽기 | Low |
| `file_write` | 파일 쓰기 | Medium |
| `file_list` | 디렉토리 목록 | Low |
| `http_get` | HTTP GET 요청 | Low |
| `http_post` | HTTP POST 요청 | Medium |
| `exec` | 명령 실행 | High |
| `git_status` | Git 상태 조회 | Low |
| `git_commit` | Git 커밋 생성 | Medium |
| `git_branch` | Git 브랜치 관리 | Medium |
| `git_diff` | Git diff 조회 | Low |
| `github_api` | GitHub API 연동 | Medium |

## 모델 라우팅

작업 유형에 따라 자동으로 적절한 모델을 선택합니다:

| 작업 유형 | 모델 티어 | 예시 모델 |
|----------|-----------|-----------|
| Classification | Fast | GPT-4o-mini, Claude Haiku |
| Summarization | Fast | GPT-4o-mini, Gemini Flash |
| Conversation | Standard | GPT-4o, Claude Sonnet |
| CodeGeneration | Standard | GPT-4o, Claude Sonnet |
| Planning | Premium | GPT-4-turbo, Claude Opus |

## 테스트

```bash
# 전체 테스트 실행
cargo test --workspace

# 통합 테스트만 실행
cargo test --test integration_test
```

## 라이선스

MIT

## 기여

이슈와 PR을 환영합니다.
