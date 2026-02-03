# Discord 연동 가이드

## 개요

Cratos를 Discord 봇으로 연동하여 서버(길드) 또는 DM에서 AI 어시스턴트를 사용할 수 있습니다.

### 주요 기능

| 기능 | 설명 |
|------|------|
| **서버 채팅** | 길드 채널에서 @멘션으로 대화 |
| **DM 지원** | 1:1 다이렉트 메시지 |
| **권한 관리** | 허용 길드/채널 설정 |
| **스레드** | 답글 컨텍스트 유지 |
| **타이핑 표시** | 응답 중 타이핑 인디케이터 |

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    Discord Server                           │
│  ┌─────────────────┐  ┌─────────────────┐                  │
│  │  #general       │  │  #dev-chat      │                  │
│  │  @Cratos 안녕   │  │                 │                  │
│  └────────┬────────┘  └─────────────────┘                  │
└───────────│────────────────────────────────────────────────┘
            │ Discord Gateway (WebSocket)
            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Cratos Server                            │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                  DiscordAdapter                          ││
│  │  ┌───────────┐  ┌───────────┐  ┌───────────────────┐   ││
│  │  │ serenity  │  │ Event     │  │ Message           │   ││
│  │  │ Client    │  │ Handler   │  │ Normalizer        │   ││
│  │  └───────────┘  └───────────┘  └───────────────────┘   ││
│  └─────────────────────────────────────────────────────────┘│
│                           │                                  │
│                           ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    Orchestrator                          ││
│  │         (LLM 처리 → 도구 실행 → 응답 생성)               ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## 설정 방법

### 1. Discord 봇 생성

1. [Discord Developer Portal](https://discord.com/developers/applications) 접속
2. "New Application" 클릭
3. 앱 이름 입력 (예: "Cratos Assistant")
4. "Bot" 탭 → "Add Bot" 클릭
5. "Reset Token" → 토큰 복사 (⚠️ 한 번만 표시됨)

### 2. 봇 권한 설정

"Bot" 탭에서 다음 권한 활성화:

```
✅ MESSAGE CONTENT INTENT (필수!)
✅ Send Messages
✅ Read Message History
✅ Add Reactions
✅ Use Slash Commands (선택)
```

### 3. 봇 초대

"OAuth2" → "URL Generator" 에서:

```
Scopes:
✅ bot
✅ applications.commands (선택)

Bot Permissions:
✅ Send Messages
✅ Read Message History
✅ Add Reactions
```

생성된 URL로 서버에 봇 초대

### 4. 환경 변수 설정

```bash
# .env
DISCORD_BOT_TOKEN=your_bot_token_here

# 선택 옵션
DISCORD_ALLOWED_GUILDS=123456789,987654321  # 허용 서버 ID (빈 값 = 모든 서버)
DISCORD_ALLOWED_CHANNELS=111222333          # 허용 채널 ID (빈 값 = 모든 채널)
DISCORD_REQUIRE_MENTION=true                # 서버에서 @멘션 필수 여부
```

## 사용 방법

### 서버 채널에서

```
사용자: @Cratos 오늘 날씨 어때?
Cratos: 현재 서울 날씨는...

사용자: @Cratos 피보나치 함수 만들어줘
Cratos: ```rust
fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n-1) + fibonacci(n-2)
    }
}
```
```

### DM에서

DM에서는 @멘션 없이 바로 대화 가능:

```
사용자: 안녕!
Cratos: 안녕하세요! 무엇을 도와드릴까요?
```

### 답글 컨텍스트

이전 메시지에 답글 달면 컨텍스트 유지:

```
사용자: 파이썬으로 리스트 정렬하는 방법 알려줘
Cratos: sorted() 함수나 .sort() 메서드를 사용...

[위 메시지에 답글]
사용자: 내림차순은?
Cratos: reverse=True 파라미터를 사용하면 됩니다...
```

## 설정 옵션

### DiscordConfig

```rust
pub struct DiscordConfig {
    /// 봇 토큰 (필수)
    pub bot_token: String,

    /// 허용된 서버 ID 목록 (빈 값 = 모든 서버 허용)
    pub allowed_guilds: Vec<u64>,

    /// 허용된 채널 ID 목록 (빈 값 = 모든 채널 허용)
    pub allowed_channels: Vec<u64>,

    /// 서버 채널에서 @멘션 필수 여부 (기본: true)
    pub require_mention: bool,
}
```

### 환경 변수

| 변수 | 필수 | 기본값 | 설명 |
|------|------|--------|------|
| `DISCORD_BOT_TOKEN` | ✅ | - | 봇 토큰 |
| `DISCORD_ALLOWED_GUILDS` | ❌ | 빈 값 | 쉼표로 구분된 서버 ID |
| `DISCORD_ALLOWED_CHANNELS` | ❌ | 빈 값 | 쉼표로 구분된 채널 ID |
| `DISCORD_REQUIRE_MENTION` | ❌ | true | false면 모든 메시지에 응답 |

## 보안

### 민감 정보 마스킹

로그에 민감 정보가 노출되지 않도록 자동 마스킹:

```rust
// 다음 패턴 포함 시 [REDACTED] 처리
const SENSITIVE_PATTERNS: &[&str] = &[
    "password", "secret", "token", "api_key",
    "bearer", "authorization", "credential", "ssh"
];
```

### 에러 메시지 정제

사용자에게 내부 에러 노출 방지:

```rust
// 내부: "Invalid token: sk-abc123..."
// 사용자: "An authentication error occurred."
```

### 권한 제한

```bash
# 특정 서버만 허용
DISCORD_ALLOWED_GUILDS=123456789

# 특정 채널만 허용
DISCORD_ALLOWED_CHANNELS=111222333,444555666
```

## 메시지 제한

Discord 메시지 길이 제한 (2000자) 자동 처리:

```rust
// 긴 응답은 자동으로 여러 메시지로 분할
let chunks: Vec<&str> = response_text
    .as_bytes()
    .chunks(2000)
    .filter_map(|chunk| std::str::from_utf8(chunk).ok())
    .collect();
```

## API 레퍼런스

### DiscordAdapter

```rust
impl DiscordAdapter {
    /// 새 어댑터 생성
    pub fn new(config: DiscordConfig) -> Self;

    /// 환경 변수에서 생성
    pub fn from_env() -> Result<Self>;

    /// 봇 실행
    pub async fn run(self: Arc<Self>, orchestrator: Arc<Orchestrator>) -> Result<()>;

    /// 서버 허용 여부 확인
    pub fn is_guild_allowed(&self, guild_id: u64) -> bool;

    /// 채널 허용 여부 확인
    pub fn is_channel_allowed(&self, channel_id: u64) -> bool;
}
```

### ChannelAdapter 구현

```rust
impl ChannelAdapter for DiscordAdapter {
    /// 메시지 전송
    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String>;

    /// 메시지 수정
    async fn edit_message(&self, channel_id: &str, message_id: &str, message: OutgoingMessage) -> Result<()>;

    /// 메시지 삭제
    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()>;

    /// 타이핑 표시
    async fn send_typing(&self, channel_id: &str) -> Result<()>;
}
```

## 문제 해결

### 봇이 응답하지 않음

1. **MESSAGE CONTENT INTENT** 활성화 확인
2. 봇이 채널 읽기/쓰기 권한 있는지 확인
3. `DISCORD_REQUIRE_MENTION=true`면 @멘션 했는지 확인
4. `DISCORD_ALLOWED_GUILDS`에 서버 ID 포함 확인

### "Invalid Token" 에러

```bash
# 토큰 형식 확인 (점 2개로 구분)
# 올바른 형식: OTk...NzY.Gh...Qw.zI...9A
echo $DISCORD_BOT_TOKEN
```

### Rate Limit

Discord API 제한에 걸리면 자동으로 재시도. 과도한 요청 시:

```
⚠️ 429 Too Many Requests
→ 잠시 후 자동 재시도
```

## 향후 계획

1. **v1.1**: 슬래시 커맨드 지원 (`/cratos ask ...`)
2. **v1.2**: 음성 채널 연동
3. **v2.0**: 임베드 메시지, 버튼 인터랙션
