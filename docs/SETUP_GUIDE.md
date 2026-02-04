# Cratos 설치 가이드

Cratos는 **내 컴퓨터**에 설치하여, 외출 중에도 Telegram으로 PC 작업을 원격 지시하는 AI 어시스턴트입니다.

## 목차

1. [개념 이해](#1-개념-이해)
2. [시스템 요구사항](#2-시스템-요구사항)
3. [Telegram 봇 만들기](#3-telegram-봇-만들기)
4. [LLM API 키 발급](#4-llm-api-키-발급)
5. [환경 변수 설정](#5-환경-변수-설정)
6. [Cratos 실행](#6-cratos-실행)
7. [정상 동작 확인](#7-정상-동작-확인)
8. [보안 설정](#8-보안-설정)
9. [문제 해결](#9-문제-해결)
10. [종료 및 재시작](#10-종료-및-재시작)

---

## 1. 개념 이해

```
┌─────────────────────────────────────────────────────────────┐
│                     내 컴퓨터 (집/회사)                      │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    Cratos                            │   │
│  │  - 파일 읽기/쓰기                                    │   │
│  │  - 명령 실행 (Docker 샌드박스)                        │   │
│  │  - Git/GitHub 작업                                   │   │
│  │  - 웹 정보 수집                                      │   │
│  │  - 10개 LLM 프로바이더 연동                          │   │
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

## 2. 시스템 요구사항

| 항목 | 최저 사양¹ | 최소 사양 | 권장 사양 |
|------|-----------|----------|----------|
| **OS** | macOS 11+, Windows 10, Ubuntu 20.04+ | macOS 12+, Windows 10+, Ubuntu 22.04+ | 최신 버전 |
| **CPU** | 2코어 | 4코어 | 8코어 이상 |
| **RAM** | 2GB (실행) / 6GB (빌드) | 6GB (실행) / 12GB (빌드) | 16GB 이상 |
| **디스크** | 3GB | 8GB | 15GB 이상 |
| **Rust** | 1.80+ | 1.80+ | 최신 stable |
| **네트워크** | 인터넷 연결 | 인터넷 연결 | 고정 IP 또는 DDNS |

> ¹ **최저 사양**: 임베딩 비활성화 시 (`cargo build --no-default-features`). 시맨틱 검색 기능 사용 불가.

> **참고**: Docker, PostgreSQL 없이 실행 가능! 데이터는 `~/.cratos/cratos.db` (SQLite)에 자동 저장됩니다.

### Ollama 로컬 LLM 사용 시

| 모델 | RAM | VRAM (GPU) | 설명 |
|------|-----|------------|------|
| Llama 3.2 1B | 2GB | 2GB | 가벼움, 빠름 |
| Llama 3.2 3B | 4GB | 4GB | 균형 |
| Qwen 2.5 7B | 8GB | 8GB | 고품질 |
| Llama 3.1 70B | 48GB | 48GB | 최고 품질 |

> **참고**: 외부 LLM API(OpenAI, Novita 등) 사용 시 GPU 불필요!

### Rust 설치 (필수)

```bash
# Rust 설치
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 설치 확인
rustc --version  # 1.80+ 필요
```

### Docker 설치 (선택사항 - 샌드박스용)

Docker는 위험한 명령 격리 실행에만 사용됩니다. 없어도 기본 기능은 작동합니다.

**macOS**:
```bash
brew install --cask docker
```

**Windows**:
- [Docker Desktop](https://www.docker.com/products/docker-desktop/) 다운로드 및 설치

**Linux**:
```bash
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER
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

### 💰 유료 옵션

#### OpenAI (GPT-4)
1. https://platform.openai.com/api-keys 접속
2. 계정 생성 또는 로그인
3. "Create new secret key" 클릭
4. 키 복사 (예: `sk-proj-xxxx...`)

#### Anthropic (Claude)
1. https://console.anthropic.com/settings/keys 접속
2. 계정 생성 또는 로그인
3. "Create Key" 클릭
4. 키 복사 (예: `sk-ant-api03-xxxx...`)

#### ZhipuAI (GLM)
1. https://open.bigmodel.cn 접속
2. 계정 생성 후 API 키 발급
3. 키 복사

#### Alibaba (Qwen)
1. https://dashscope.console.aliyun.com 접속
2. 계정 생성 후 API 키 발급
3. 키 복사

### 🆓 무료 옵션

#### OpenRouter (추천!)
1. https://openrouter.ai 접속
2. GitHub/Google로 가입 (간단!)
3. API Keys 메뉴에서 키 발급
4. **무료 모델**: Qwen3-32B, Llama 3.2 (1000회/일)

#### Novita AI (무료 가입)
1. https://novita.ai 접속
2. 무료 가입
3. API Keys 발급
4. **무료 모델**: Llama 3.2, Qwen2.5-7B, GLM-4-9B

#### Ollama (완전 무료, 로컬)
별도 API 키 없이 로컬에서 무료로 사용:
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
# LLM API 키 (최소 하나 선택)
# ================================

# 유료: OpenAI
OPENAI_API_KEY=sk-proj-your-key-here

# 유료: Anthropic
ANTHROPIC_API_KEY=sk-ant-api03-your-key-here

# 유료: ZhipuAI GLM
BIGMODEL_API_KEY=your-bigmodel-key-here

# 유료: Alibaba Qwen
DASHSCOPE_API_KEY=your-dashscope-key-here

# 무료: OpenRouter (추천!)
OPENROUTER_API_KEY=sk-or-your-key-here

# 무료: Novita AI
NOVITA_API_KEY=your-novita-key-here

# 무료: Ollama (키 불필요, 아래 주석 해제)
# OLLAMA_BASE_URL=http://host.docker.internal:11434

# ================================
# 선택사항 (기본값 사용 가능)
# ================================
# REDIS_URL=redis://localhost:6379   # 없으면 메모리 세션 사용
# CRATOS_DATA_DIR=~/.cratos          # 데이터 저장 경로
RUST_LOG=cratos=info,tower_http=info
```

> **참고**: `DATABASE_URL`은 더 이상 필요 없습니다. 내장 SQLite (`~/.cratos/cratos.db`)를 사용합니다.

### 💡 비용 절감 팁

무료로 시작하려면:
1. **OpenRouter** 키만 발급 (GitHub 로그인으로 1분 완료)
2. `.env`에 `OPENROUTER_API_KEY`만 설정
3. 하루 1000회 무료 사용!

---

## 6. Cratos 실행

```bash
# 빌드 (첫 실행 시 약 5~10분)
cargo build --release

# 실행
cargo run --release

# 또는 한 번에
cargo run
```

정상 시작 시 다음과 같이 표시됩니다:
```
Starting Cratos AI Assistant v0.1.0
Configuration loaded
Data directory: /Users/yourname/.cratos
SQLite event store initialized at /Users/yourname/.cratos/cratos.db
LLM provider initialized: anthropic
Tool registry initialized with 11 tools
Telegram adapter started
HTTP server listening on http://127.0.0.1:9742
```

> **참고**: 데이터베이스 파일(`~/.cratos/cratos.db`)은 자동으로 생성됩니다.

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

## 8. 보안 설정

Cratos는 보안을 위한 여러 기능을 기본 제공합니다.

### 8.1 샌드박스 설정

`config/local.toml` 생성:

```toml
[security]
# strict: 모든 도구 격리
# moderate: 위험 도구만 격리 (기본)
# disabled: 개발 모드
sandbox_policy = "moderate"

[security.sandbox]
default_network = "none"    # 네트워크 차단
max_memory_mb = 512         # 메모리 제한
max_cpu_percent = 50        # CPU 제한
```

### 8.2 자격증명 보안

API 키를 OS 키체인에 저장하면 더 안전합니다:

```toml
[security]
# auto: 플랫폼에 따라 자동 선택
# keychain: macOS Keychain
# secret_service: Linux
# encrypted_file: 암호화 파일
credential_backend = "auto"
```

### 8.3 프롬프트 인젝션 방어

악의적인 프롬프트 공격을 자동 차단합니다:

```toml
[security.injection]
# 차단 임계값: info, low, medium, high, critical
block_threshold = "medium"
```

---

## 9. 문제 해결

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

### 데이터베이스 오류

SQLite는 내장되어 있어 별도 설정이 필요 없습니다. 문제가 있다면:

```bash
# 데이터 디렉토리 확인
ls -la ~/.cratos/

# 데이터베이스 파일 확인
sqlite3 ~/.cratos/cratos.db ".tables"

# 초기화 (데이터 삭제)
rm ~/.cratos/cratos.db
```

### 메모리 부족 (Ollama 사용 시)

더 작은 모델 사용:
```bash
ollama pull llama3.2:1b   # 1B 모델 (2GB RAM)
```

---

## 10. 종료 및 재시작

### 종료

터미널에서 `Ctrl+C`를 누르거나:

```bash
# 프로세스 찾아서 종료
pkill -f "cratos"
```

### 재시작

```bash
cargo run --release
```

### 초기화 (모든 데이터 삭제)

```bash
rm -rf ~/.cratos/
```

### 백그라운드 실행 (선택)

```bash
# nohup 사용
nohup cargo run --release > cratos.log 2>&1 &

# 또는 systemd 서비스 등록 (Linux)
```

---

## 다음 단계

설치가 완료되었습니다! [사용 가이드](./USER_GUIDE.md)에서 다양한 기능을 확인하세요.

### 추천 첫 사용

```
나: 안녕, 넌 뭘 할 수 있어?
```

Cratos가 할 수 있는 일들을 안내받을 수 있습니다.
