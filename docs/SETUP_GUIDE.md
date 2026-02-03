# Cratos 설치 가이드

Cratos는 **내 컴퓨터**에 설치하여, 외출 중에도 Telegram으로 PC 작업을 원격 지시하는 AI 어시스턴트입니다.

## 목차

1. [개념 이해](#1-개념-이해)
2. [사전 요구사항](#2-사전-요구사항)
3. [Telegram 봇 만들기](#3-telegram-봇-만들기)
4. [LLM API 키 발급](#4-llm-api-키-발급)
5. [환경 변수 설정](#5-환경-변수-설정)
6. [Cratos 실행](#6-cratos-실행)
7. [정상 동작 확인](#7-정상-동작-확인)
8. [문제 해결](#8-문제-해결)
9. [종료 및 재시작](#9-종료-및-재시작)

---

## 1. 개념 이해

```
┌─────────────────────────────────────────────────────────────┐
│                     내 컴퓨터 (집/회사)                      │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    Cratos                            │   │
│  │  - 파일 읽기/쓰기                                    │   │
│  │  - 명령 실행                                         │   │
│  │  - Git/GitHub 작업                                   │   │
│  │  - 웹 정보 수집                                      │   │
│  └─────────────────────────────────────────────────────┘   │
│                           ↑                                 │
│                           │ Telegram API                    │
└───────────────────────────┼─────────────────────────────────┘
                            │
                            ↓
                   ┌─────────────────┐
                   │  Telegram 서버   │
                   └─────────────────┘
                            ↑
                            │
                   ┌─────────────────┐
                   │    내 스마트폰    │
                   │   (어디서든!)     │
                   └─────────────────┘
```

**핵심**: Cratos는 내 컴퓨터에서 실행됩니다. 나만의 Telegram 봇을 통해 어디서든 내 PC에 명령할 수 있습니다.

---

## 2. 사전 요구사항

### Docker 설치 (권장)

가장 쉬운 방법입니다. Docker만 설치하면 됩니다.

**macOS**:
```bash
brew install --cask docker
# Docker Desktop 앱 실행
```

**Windows**:
- [Docker Desktop](https://www.docker.com/products/docker-desktop/) 다운로드 및 설치

**Linux**:
```bash
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER
# 로그아웃 후 재로그인
```

설치 확인:
```bash
docker --version
docker-compose --version
```

---

## 3. Telegram 봇 만들기

나만의 Telegram 봇을 만들어야 합니다. **5분이면 완료됩니다.**

### 3.1 BotFather에서 봇 생성

1. Telegram 앱 열기 (핸드폰 또는 데스크톱)
2. 검색창에 `@BotFather` 입력
3. 파란색 체크 표시된 공식 BotFather 선택
4. `/newbot` 입력

### 3.2 봇 이름 정하기

```
BotFather: Alright, a new bot. How are we going to call it?
나: My Personal Assistant
```

봇 표시 이름을 입력합니다 (한글 가능).

### 3.3 봇 유저네임 정하기

```
BotFather: Good. Now let's choose a username for your bot.
나: my_personal_cratos_bot
```

**중요**: 반드시 `_bot`으로 끝나야 합니다.

### 3.4 토큰 복사

```
BotFather: Done! Congratulations on your new bot.
Use this token to access the HTTP API:
7123456789:AAHxxxxxxxxxxxxxxxxxxxxxxxxxx
```

이 토큰을 안전한 곳에 복사해 둡니다.

⚠️ **경고**: 이 토큰은 절대 공개하면 안 됩니다!

---

## 4. LLM API 키 발급

Cratos가 AI 기능을 사용하려면 LLM API 키가 필요합니다.

### 옵션 1: OpenAI (GPT-4)

1. https://platform.openai.com/api-keys 접속
2. 계정 생성 또는 로그인
3. "Create new secret key" 클릭
4. 키 복사 (예: `sk-proj-xxxx...`)

### 옵션 2: Anthropic (Claude)

1. https://console.anthropic.com/settings/keys 접속
2. 계정 생성 또는 로그인
3. "Create Key" 클릭
4. 키 복사 (예: `sk-ant-api03-xxxx...`)

### 옵션 3: Ollama (무료, 로컬)

별도 API 키 없이 로컬에서 무료로 사용 가능:
```bash
# Ollama 설치 (macOS)
brew install ollama

# 모델 다운로드
ollama pull llama3.2

# Ollama 실행
ollama serve
```

---

## 5. 환경 변수 설정

### 5.1 Cratos 다운로드

```bash
git clone https://github.com/cratos/cratos.git
cd cratos
```

### 5.2 설정 파일 생성

```bash
cp .env.example .env
```

### 5.3 .env 파일 편집

```bash
# 텍스트 편집기로 열기
nano .env
# 또는
code .env
```

필수 항목만 채우면 됩니다:

```bash
# ================================
# 필수: Telegram 봇 토큰
# ================================
TELEGRAM_BOT_TOKEN=7123456789:AAHxxxxxxxxxxxxxxxxxxxxxxxxxx

# ================================
# 필수: LLM API 키 (최소 하나)
# ================================
# OpenAI 사용 시
OPENAI_API_KEY=sk-proj-your-key-here

# 또는 Anthropic 사용 시
ANTHROPIC_API_KEY=sk-ant-api03-your-key-here

# 또는 Ollama 사용 시 (키 불필요, 아래 주석 해제)
# OLLAMA_BASE_URL=http://host.docker.internal:11434

# ================================
# 아래는 수정하지 않아도 됩니다
# ================================
DATABASE_URL=postgres://cratos:cratos@postgres:5432/cratos
REDIS_URL=redis://redis:6379
RUST_LOG=cratos=info,tower_http=info
```

---

## 6. Cratos 실행

```bash
# 첫 실행 (빌드 포함, 약 5~10분)
docker-compose up --build -d
```

실행 후 로그 확인:
```bash
docker-compose logs -f cratos
```

정상 시작 시 다음과 같이 표시됩니다:
```
Starting Cratos AI Assistant v0.1.0
Configuration loaded
Database connection established
Telegram adapter started
HTTP server listening on http://0.0.0.0:8080
```

---

## 7. 정상 동작 확인

### 7.1 헬스체크

```bash
curl http://localhost:9742/health
```

응답:
```json
{"status":"healthy","version":"0.1.0"}
```

### 7.2 Telegram에서 테스트

1. Telegram 앱 열기
2. 검색창에 내 봇 유저네임 입력 (예: `@my_personal_cratos_bot`)
3. 봇 선택 후 "시작" 버튼 클릭
4. 메시지 전송: `안녕`

10초 내에 응답이 오면 성공입니다!

### 7.3 기본 명령 테스트

```
나: 현재 디렉토리 파일 목록 보여줘
봇: (파일 목록 응답)

나: 오늘 날짜 알려줘
봇: (날짜 응답)
```

---

## 8. 문제 해결

### 봇이 응답하지 않음

```bash
# 1. 로그 확인
docker-compose logs cratos

# 2. 컨테이너 상태 확인
docker-compose ps

# 3. 재시작
docker-compose restart cratos
```

### "Unauthorized" 또는 API 키 오류

1. `.env` 파일의 API 키 확인
2. 키 앞뒤 공백 제거
3. 재시작: `docker-compose restart cratos`

### 포트 충돌

다른 프로그램과 포트가 겹칠 경우:

```yaml
# docker-compose.yml 수정
ports:
  - "9999:8080"  # 9742 대신 다른 포트
```

### 데이터베이스 연결 오류

```bash
# PostgreSQL 상태 확인
docker-compose logs postgres

# 데이터베이스 재시작
docker-compose restart postgres
```

---

## 9. 종료 및 재시작

### 일시 중지

```bash
docker-compose stop
```

### 완전 종료

```bash
docker-compose down
```

### 재시작

```bash
docker-compose up -d
```

### 초기화 (모든 데이터 삭제)

```bash
docker-compose down -v
```

---

## 다음 단계

설치가 완료되었습니다! [사용 가이드](./USER_GUIDE.md)에서 다양한 기능을 확인하세요.

### 추천 첫 사용

```
나: 안녕, 넌 뭘 할 수 있어?
```

Cratos가 할 수 있는 일들을 안내받을 수 있습니다.
