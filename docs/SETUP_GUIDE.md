# Cratos 설치 및 설정 가이드

이 문서는 Cratos를 처음 설치하고 실행하기까지의 전체 과정을 설명합니다.

## 목차

1. [사전 요구사항](#1-사전-요구사항)
2. [Telegram 봇 생성](#2-telegram-봇-생성)
3. [LLM API 키 발급](#3-llm-api-키-발급)
4. [환경 변수 설정](#4-환경-변수-설정)
5. [서비스 실행](#5-서비스-실행)
6. [헬스체크](#6-헬스체크)
7. [문제 해결](#7-문제-해결)
8. [서비스 중지](#8-서비스-중지)
9. [로컬 개발 모드](#9-로컬-개발-모드-선택)

---

## 1. 사전 요구사항

### 필수 소프트웨어

```bash
# Docker 설치 확인
docker --version
# 예: Docker version 24.0.0

# Docker Compose 확인
docker-compose --version
# 예: Docker Compose version v2.20.0
```

### 선택 (로컬 개발용)

```bash
# Rust 설치 확인
rustc --version
# 예: rustc 1.75.0
```

---

## 2. Telegram 봇 생성

Telegram에서 봇을 생성하여 토큰을 발급받습니다.

### 단계별 진행

1. **Telegram 앱 열기** (모바일 또는 데스크톱)

2. **@BotFather 검색**
   - 검색창에 `BotFather` 입력
   - 파란색 체크마크가 있는 공식 계정 선택

3. **새 봇 생성**
   ```
   /newbot
   ```

4. **봇 이름 입력**
   - 사용자에게 표시될 이름
   - 예: `My Cratos Assistant`

5. **봇 유저네임 입력**
   - 고유한 이름 (반드시 `_bot`으로 끝나야 함)
   - 예: `my_cratos_bot`

6. **봇 토큰 복사**
   - BotFather가 발급한 토큰을 복사
   - 형식: `7123456789:AAHxxxxxxxxxxxxxxxxxxxxxxxxxx`
   - **주의: 이 토큰을 외부에 노출하지 마세요!**

---

## 3. LLM API 키 발급

최소 하나의 LLM 프로바이더 API 키가 필요합니다.

### OpenAI API 키

1. https://platform.openai.com/api-keys 접속
2. OpenAI 계정으로 로그인
3. "Create new secret key" 클릭
4. 키 이름 입력 (예: `cratos`)
5. 키 복사 (예: `sk-proj-xxxx...`)

**비용**: 사용량 기반 과금 (GPT-4o: ~$2.5/1M 입력 토큰)

### Anthropic API 키

1. https://console.anthropic.com/settings/keys 접속
2. Anthropic 계정으로 로그인
3. "Create Key" 클릭
4. 키 복사 (예: `sk-ant-api03-xxxx...`)

**비용**: 사용량 기반 과금 (Claude Sonnet: ~$3/1M 입력 토큰)

---

## 4. 환경 변수 설정

### 4.1 .env 파일 생성

```bash
cd /path/to/cratos

# 예제 파일 복사
cp .env.example .env
```

### 4.2 .env 파일 편집

```bash
# 편집기로 열기
vim .env
# 또는
code .env
```

### 4.3 .env 파일 내용

```bash
# ================================
# 데이터베이스 (Docker 사용 시 수정 불필요)
# ================================
DATABASE_URL=postgres://cratos:cratos@postgres:5432/cratos
REDIS_URL=redis://redis:6379

# ================================
# LLM API 키 (최소 하나 필수)
# ================================
OPENAI_API_KEY=sk-proj-your-openai-key-here
ANTHROPIC_API_KEY=sk-ant-api03-your-anthropic-key-here

# ================================
# Telegram 봇 토큰 (필수)
# ================================
TELEGRAM_BOT_TOKEN=7123456789:AAHxxxxxxxxxxxxxxxxxxxxxxxxxx

# ================================
# Slack (선택, 사용 안하면 비워두기)
# ================================
SLACK_BOT_TOKEN=
SLACK_SIGNING_SECRET=

# ================================
# 로깅 레벨
# ================================
RUST_LOG=cratos=info,tower_http=info
```

---

## 5. 서비스 실행

### 5.1 전체 스택 빌드 및 실행

```bash
# 빌드 및 백그라운드 실행
docker-compose up --build -d
```

첫 빌드는 약 5-10분 소요됩니다.

### 5.2 로그 확인

```bash
# 실시간 로그 확인
docker-compose logs -f cratos
```

### 5.3 정상 시작 로그

```
Starting Cratos AI Assistant v0.1.0
Configuration loaded
Database connection established
Database migrations completed
Event store initialized
LLM provider initialized: anthropic
Tool registry initialized with 11 tools
Redis session store initialized
Orchestrator initialized
Telegram adapter started
HTTP server listening on http://0.0.0.0:8080
```

---

## 6. 헬스체크

### 기본 헬스체크

```bash
curl http://localhost:9742/health
```

응답:
```json
{"status":"healthy","version":"0.1.0"}
```

### 상세 헬스체크

```bash
curl http://localhost:9742/health/detailed
```

응답:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "checks": {
    "database": {"status": "healthy", "latency_ms": 2},
    "redis": {"status": "healthy", "latency_ms": 1}
  }
}
```

---

## 7. 문제 해결

### 봇이 응답하지 않을 때

```bash
# 1. 로그 확인
docker-compose logs -f cratos

# 2. 컨테이너 상태 확인
docker-compose ps

# 3. 재시작
docker-compose restart cratos
```

### 데이터베이스 연결 오류

```bash
# PostgreSQL 로그 확인
docker-compose logs postgres

# DB 직접 접속 테스트
docker-compose exec postgres psql -U cratos -d cratos -c "SELECT 1"
```

### API 키 오류

**증상**: 로그에 `401 Unauthorized` 또는 `invalid API key`

**해결**:
1. `.env` 파일의 API 키 확인
2. API 키가 만료되지 않았는지 확인
3. 키 앞뒤 공백 제거
4. 컨테이너 재시작: `docker-compose restart cratos`

### 포트 충돌

**증상**: `port is already allocated`

**해결**: `docker-compose.yml`에서 포트 변경
```yaml
ports:
  - "9743:8080"  # 9742 대신 다른 포트 사용
```

---

## 8. 서비스 중지

### 일반 중지

```bash
docker-compose down
```

### 데이터까지 완전 삭제 (초기화)

```bash
docker-compose down -v
```

---

## 9. 로컬 개발 모드 (선택)

Docker 대신 로컬에서 직접 실행하려면:

### 9.1 DB와 Redis만 Docker로 실행

```bash
docker-compose up -d postgres redis
```

### 9.2 .env 수정 (로컬 포트 사용)

```bash
DATABASE_URL=postgres://cratos:cratos@localhost:54329/cratos
REDIS_URL=redis://localhost:63791
```

### 9.3 마이그레이션 실행

```bash
cargo sqlx migrate run
```

### 9.4 실행

```bash
cargo run
```

---

## 다음 단계

설치가 완료되면 [사용자 가이드](./USER_GUIDE.md)를 참고하여 Cratos를 사용해 보세요.
