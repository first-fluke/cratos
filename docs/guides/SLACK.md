# Slack 연동 가이드

## 개요

Cratos를 Slack 봇으로 연동하여 워크스페이스 채널 또는 DM에서 AI 어시스턴트를 사용할 수 있습니다. Socket Mode를 통한 실시간 이벤트 처리와 강력한 보안 기능을 제공합니다.

### 주요 기능

| 기능 | 설명 |
|------|------|
| **채널 대화** | 공개/비공개 채널에서 @멘션으로 대화 |
| **DM 지원** | 1:1 다이렉트 메시지 (멘션 불필요) |
| **Socket Mode** | 방화벽 뒤에서도 실시간 연결 |
| **스레드 지원** | 대화 컨텍스트 유지 |
| **권한 관리** | 워크스페이스/채널별 접근 제어 |
| **요청 서명 검증** | HMAC-SHA256 기반 보안 |
| **인터랙티브 버튼** | Block Kit 기반 UI 요소 |

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    Slack Workspace                           │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │  #general       │  │  @Cratos (DM)   │                   │
│  │  @Cratos 안녕   │  │  "작업 요청"    │                   │
│  └────────┬────────┘  └────────┬────────┘                   │
└───────────│────────────────────│────────────────────────────┘
            │                    │
            ▼                    ▼
┌─────────────────────────────────────────────────────────────┐
│              Slack API (Socket Mode / Events API)            │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  WebSocket Connection (wss://wss-primary.slack.com)    │ │
│  └────────────────────────────────────────────────────────┘ │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                     Cratos Server                            │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    SlackAdapter                          ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐  ││
│  │  │ slack-       │  │ Signature    │  │ Message       │  ││
│  │  │ morphism     │  │ Verifier     │  │ Normalizer    │  ││
│  │  │ Client       │  │ (HMAC-SHA256)│  │               │  ││
│  │  └──────────────┘  └──────────────┘  └───────────────┘  ││
│  └─────────────────────────────────────────────────────────┘│
│                           │                                  │
│                           ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    Orchestrator                          ││
│  │         (LLM 처리 -> 도구 실행 -> 응답 생성)             ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## 설정 방법

### 1. Slack App 생성

1. [Slack API Portal](https://api.slack.com/apps) 접속
2. "Create New App" 클릭
3. "From scratch" 선택
4. App 이름 입력 (예: "Cratos Assistant")
5. 워크스페이스 선택 후 "Create App"

### 2. OAuth 권한 설정

**OAuth & Permissions** 탭에서 다음 Bot Token Scopes 추가:

```
필수 스코프:
✅ chat:write          # 메시지 전송
✅ chat:write.public   # 공개 채널에 메시지 전송
✅ im:history          # DM 히스토리 읽기
✅ im:read             # DM 채널 정보 읽기
✅ im:write            # DM 전송
✅ channels:history    # 공개 채널 히스토리 읽기
✅ channels:read       # 공개 채널 정보 읽기
✅ groups:history      # 비공개 채널 히스토리 읽기
✅ groups:read         # 비공개 채널 정보 읽기
✅ users:read          # 사용자 정보 읽기

선택 스코프:
✅ app_mentions:read   # 앱 멘션 이벤트 구독
✅ reactions:write     # 리액션 추가
✅ files:write         # 파일 업로드
```

### 3. Socket Mode 활성화

**Socket Mode** 탭에서:

1. "Enable Socket Mode" 토글 ON
2. App-Level Token 생성:
   - Token Name: "cratos-socket" (임의 이름)
   - Scope: `connections:write` 추가
   - "Generate" 클릭
3. `xapp-...` 형식의 토큰 복사 (안전하게 저장)

### 4. Event Subscriptions 설정

**Event Subscriptions** 탭에서:

1. "Enable Events" 토글 ON
2. Socket Mode 사용 시 Request URL 불필요
3. **Subscribe to bot events** 에서 추가:

```
✅ message.channels     # 공개 채널 메시지
✅ message.groups       # 비공개 채널 메시지
✅ message.im           # DM 메시지
✅ app_mention          # @멘션 이벤트
```

### 5. 토큰 수집

필요한 토큰 3개:

| 토큰 | 위치 | 형식 |
|------|------|------|
| Bot Token | OAuth & Permissions | `xoxb-...` |
| App Token | Basic Information > App-Level Tokens | `xapp-...` |
| Signing Secret | Basic Information > App Credentials | 32자 hex |

### 6. 앱 설치

1. **Install App** 탭으로 이동
2. "Install to Workspace" 클릭
3. 권한 검토 후 "Allow"

### 7. 환경 변수 설정

```bash
# .env 파일
# 필수 설정
SLACK_BOT_TOKEN=xoxb-1234567890-1234567890123-AbCdEfGhIjKlMnOpQrStUvWx
SLACK_APP_TOKEN=xapp-1-A1234567890-1234567890123-abcdefghijklmnopqrstuvwxyz1234567890
SLACK_SIGNING_SECRET=abcdef1234567890abcdef1234567890

# 선택 설정
SLACK_ALLOWED_WORKSPACES=T1234567890,T0987654321  # 허용 워크스페이스 (빈 값 = 모두 허용)
SLACK_ALLOWED_CHANNELS=C1234567890,C0987654321    # 허용 채널 (빈 값 = 모두 허용)
SLACK_MENTIONS_ONLY=true                          # true면 @멘션/DM만 응답
```

## Socket Mode vs Events API

### Socket Mode (권장)

```
장점:
✅ 방화벽/NAT 뒤에서 동작
✅ 공개 URL 불필요
✅ 즉시 연결 (URL 검증 불필요)
✅ 개발 환경에 적합

단점:
❌ 동시 연결 수 제한
❌ 대규모 배포에 부적합
```

### Events API (HTTP Webhook)

```
장점:
✅ 무제한 스케일링
✅ 로드 밸런싱 가능
✅ 대규모 배포에 적합

단점:
❌ 공개 HTTPS 엔드포인트 필요
❌ Request URL 검증 필요
❌ 서명 검증 필수
```

### 연결 모드 선택 가이드

| 상황 | 권장 모드 |
|------|----------|
| 개발/테스트 | Socket Mode |
| 소규모 팀 (< 100명) | Socket Mode |
| 방화벽 내부 서버 | Socket Mode |
| 대규모 배포 | Events API |
| 고가용성 필요 | Events API |

## 사용 방법

### 채널에서 대화

```
사용자: @Cratos 오늘 주요 작업 정리해줘
Cratos: 오늘의 주요 작업을 정리했습니다:
        1. API 문서 업데이트
        2. 테스트 커버리지 개선
        3. 성능 최적화 검토
```

### DM에서 대화

DM에서는 @멘션 없이 바로 대화 가능:

```
사용자: 코드 리뷰 부탁해
Cratos: 네, 어떤 PR을 리뷰할까요?

사용자: #123
Cratos: PR #123을 검토했습니다...
```

### 스레드 컨텍스트

스레드에서 대화하면 컨텍스트 유지:

```
사용자: @Cratos 피보나치 함수 만들어줘
Cratos: fn fibonacci(n: u64) -> u64 { ... }

[스레드에서]
사용자: 반복문으로 바꿔줘
Cratos: fn fibonacci_iter(n: u64) -> u64 { ... }
```

## 설정 옵션

### SlackConfig

```rust
pub struct SlackConfig {
    /// Bot Token (xoxb-...)
    /// OAuth & Permissions에서 발급
    pub bot_token: String,

    /// App Token for Socket Mode (xapp-...)
    /// Basic Information > App-Level Tokens에서 발급
    pub app_token: String,

    /// Signing Secret for request verification
    /// Basic Information > App Credentials에서 확인
    pub signing_secret: String,

    /// 허용된 워크스페이스 ID 목록
    /// 빈 배열 = 모든 워크스페이스 허용
    pub allowed_workspaces: Vec<String>,

    /// 허용된 채널 ID 목록
    /// 빈 배열 = 모든 채널 허용
    pub allowed_channels: Vec<String>,

    /// 멘션 전용 모드
    /// true: @멘션 또는 DM에서만 응답
    /// false: 모든 메시지에 응답
    pub mentions_only: bool,
}
```

### 프로그래매틱 설정

```rust
use cratos_channels::slack::{SlackAdapter, SlackConfig};

// Builder 패턴
let config = SlackConfig::new(
    "xoxb-your-bot-token",
    "xapp-your-app-token",
    "your-signing-secret"
)
.with_allowed_workspaces(vec!["T1234567890".to_string()])
.with_allowed_channels(vec!["C1234567890".to_string(), "C0987654321".to_string()])
.with_mentions_only(true);

let adapter = SlackAdapter::new(config);

// 또는 환경 변수에서 생성
let adapter = SlackAdapter::from_env()?;
```

### 환경 변수

| 변수 | 필수 | 기본값 | 설명 |
|------|------|--------|------|
| `SLACK_BOT_TOKEN` | O | - | Bot User OAuth Token (xoxb-...) |
| `SLACK_APP_TOKEN` | O | - | App-Level Token (xapp-...) |
| `SLACK_SIGNING_SECRET` | O | - | 요청 서명 검증용 시크릿 |
| `SLACK_ALLOWED_WORKSPACES` | X | 빈 값 | 쉼표로 구분된 워크스페이스 ID |
| `SLACK_ALLOWED_CHANNELS` | X | 빈 값 | 쉼표로 구분된 채널 ID |
| `SLACK_MENTIONS_ONLY` | X | true | "true" 또는 "1"이면 멘션 모드 |

## 보안

### 요청 서명 검증 (HMAC-SHA256)

Slack은 모든 HTTP 요청에 서명을 포함합니다. Cratos는 이를 검증하여 위조 요청을 차단합니다.

```rust
// 검증 프로세스
pub fn verify_signature(&self, timestamp: &str, body: &str, signature: &str) -> Result<()> {
    // 1. 타임스탬프 검증 (5분 이내)
    // 2. HMAC-SHA256 서명 계산
    // 3. 상수 시간 비교 (타이밍 공격 방지)
}
```

#### 서명 검증 흐름

```
1. Slack 요청 수신
   Headers:
   - X-Slack-Request-Timestamp: 1531420618
   - X-Slack-Signature: v0=a2114d57b48eac39...

2. Base String 생성
   sig_basestring = "v0:{timestamp}:{body}"

3. HMAC-SHA256 계산
   expected = HMAC-SHA256(signing_secret, sig_basestring)

4. 서명 비교 (상수 시간)
   if signature == "v0={expected_hex}" -> OK
   else -> Reject
```

### 리플레이 공격 방지

```rust
// 타임스탬프가 5분 이상 오래된 요청 거부
const MAX_TIMESTAMP_AGE_SECS: u64 = 300;

let age = now.abs_diff(request_timestamp);
if age > MAX_TIMESTAMP_AGE_SECS {
    return Err("Request timestamp is too old");
}
```

### 상수 시간 비교

타이밍 공격을 방지하기 위해 상수 시간 비교 사용:

```rust
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}
```

### 권한 제어

```bash
# 특정 워크스페이스만 허용
SLACK_ALLOWED_WORKSPACES=T1234567890

# 특정 채널만 허용
SLACK_ALLOWED_CHANNELS=C1234567890,C0987654321

# DM 채널은 'D'로 시작 (자동 감지)
# D1234567890 -> DM으로 인식, 멘션 불필요
```

## API 레퍼런스

### SlackAdapter

```rust
impl SlackAdapter {
    /// 새 어댑터 생성
    pub fn new(config: SlackConfig) -> Self;

    /// 환경 변수에서 생성
    pub fn from_env() -> Result<Self>;

    /// 봇 실행 (Socket Mode)
    pub async fn run(self: Arc<Self>, orchestrator: Arc<Orchestrator>) -> Result<()>;

    /// 워크스페이스 허용 여부 확인
    pub fn is_workspace_allowed(&self, workspace_id: &str) -> bool;

    /// 채널 허용 여부 확인
    pub fn is_channel_allowed(&self, channel_id: &str) -> bool;

    /// 봇 멘션 확인
    pub async fn is_bot_mentioned(&self, text: &str) -> bool;

    /// 봇 User ID 조회
    pub async fn get_bot_user_id(&self) -> Option<String>;

    /// 요청 서명 검증
    pub fn verify_signature(
        &self,
        timestamp: &str,
        body: &str,
        signature: &str
    ) -> Result<()>;

    /// 웹훅 요청 검증 (헤더 포함)
    pub fn verify_webhook_request(
        &self,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<()>;

    /// 메시지 처리 (웹훅/소켓 모드에서 호출)
    pub async fn process_message(
        &self,
        orchestrator: &Orchestrator,
        channel: &str,
        user: &str,
        text: &str,
        ts: &str,
        thread_ts: Option<&str>,
    ) -> Result<Option<String>>;

    /// 메시지 정규화
    pub async fn normalize_message(
        &self,
        channel_id: &str,
        user_id: &str,
        text: &str,
        ts: &str,
        thread_ts: Option<&str>,
    ) -> Option<NormalizedMessage>;
}
```

### ChannelAdapter 구현

```rust
impl ChannelAdapter for SlackAdapter {
    /// 채널 타입 반환
    fn channel_type(&self) -> ChannelType;

    /// 메시지 전송
    async fn send_message(
        &self,
        channel_id: &str,
        message: OutgoingMessage
    ) -> Result<String>;

    /// 메시지 수정
    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        message: OutgoingMessage
    ) -> Result<()>;

    /// 메시지 삭제
    async fn delete_message(
        &self,
        channel_id: &str,
        message_id: &str
    ) -> Result<()>;

    /// 타이핑 표시 (Slack 봇은 미지원)
    async fn send_typing(&self, channel_id: &str) -> Result<()>;
}
```

### OutgoingMessage 사용

```rust
use cratos_channels::message::OutgoingMessage;

// 기본 텍스트 메시지
let message = OutgoingMessage::text("Hello, World!");

// 스레드 답글
let reply = OutgoingMessage::text("Thread reply")
    .in_thread("1234567890.123456".to_string());

// 버튼 포함 (Block Kit)
let interactive = OutgoingMessage::text("Choose an option:")
    .with_buttons(vec![
        MessageButton::new("Option A", "option_a"),
        MessageButton::new("Option B", "option_b"),
    ]);
```

## 문제 해결

### 봇이 응답하지 않음

1. **Event Subscriptions 확인**
   - `message.channels`, `message.im`, `app_mention` 이벤트 구독 확인

2. **OAuth 스코프 확인**
   - `chat:write`, `channels:history`, `im:history` 등 필수 스코프 확인

3. **채널 초대 확인**
   - 비공개 채널은 봇을 명시적으로 초대해야 함
   - `/invite @Cratos` 실행

4. **멘션 모드 확인**
   ```bash
   # 공개 채널에서 모든 메시지에 응답하려면
   SLACK_MENTIONS_ONLY=false
   ```

### "invalid_auth" 에러

```bash
# 토큰 형식 확인
# Bot Token: xoxb-로 시작
# App Token: xapp-로 시작

echo $SLACK_BOT_TOKEN | head -c 5  # xoxb-
echo $SLACK_APP_TOKEN | head -c 5  # xapp-
```

### "missing_scope" 에러

```
Error: missing_scope
Needed: chat:write

해결:
1. api.slack.com/apps에서 앱 선택
2. OAuth & Permissions > Scopes
3. 필요한 스코프 추가
4. 앱 재설치 (Install App)
```

### Socket Mode 연결 실패

```bash
# App Token에 connections:write 스코프 확인
# Basic Information > App-Level Tokens에서 확인

# Socket Mode 활성화 확인
# Settings > Socket Mode > Enable Socket Mode: ON
```

### 서명 검증 실패

```
Error: Invalid request signature

가능한 원인:
1. SLACK_SIGNING_SECRET 값이 잘못됨
2. 요청 body가 변형됨 (인코딩 문제)
3. 타임스탬프가 5분 이상 지남 (리플레이 공격 감지)

확인:
- Basic Information > App Credentials > Signing Secret
- 공백 없이 정확히 복사했는지 확인
```

### Rate Limiting

```
Error: ratelimited

Slack API 제한:
- Tier 1: 1+ per minute
- Tier 2: 20+ per minute
- Tier 3: 50+ per minute
- Tier 4: 100+ per minute

해결:
- chat.postMessage: Tier 3 (채널당 1msg/sec 권장)
- 재시도 시 Retry-After 헤더 확인
```

### 채널 ID 찾기

```
방법 1: 채널 우클릭 > "Copy link"
https://workspace.slack.com/archives/C1234567890
                                      ^^^^^^^^^^^
                                      채널 ID

방법 2: 채널 상세정보 > 하단에 ID 표시

채널 ID 형식:
- C... : 공개 채널
- G... : 비공개 채널
- D... : DM
- T... : 워크스페이스
```

## 향후 계획

1. **v1.1**: 슬래시 커맨드 지원 (`/cratos ask ...`)
2. **v1.2**: 모달/홈 탭 지원
3. **v1.3**: 파일 업로드/다운로드
4. **v2.0**: Workflow Builder 연동
