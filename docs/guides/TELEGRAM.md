# Telegram 연동 가이드

## 개요

Cratos를 Telegram 봇으로 연동하여 개인 채팅 또는 그룹에서 AI 어시스턴트를 사용할 수 있습니다.

### 주요 기능

| 기능 | 설명 |
|------|------|
| **개인 채팅** | 1:1 다이렉트 메시지 |
| **그룹 지원** | 그룹/슈퍼그룹에서 @멘션으로 대화 |
| **권한 관리** | 허용 사용자/그룹 설정 |
| **답글 컨텍스트** | 답글 체인으로 대화 흐름 유지 |
| **타이핑 표시** | 응답 중 타이핑 인디케이터 |
| **첨부 파일** | 이미지, 문서 첨부 지원 |
| **인라인 키보드** | 버튼 기반 인터랙션 |
| **마크다운** | HTML 형식 응답 (MarkdownV2에서 마이그레이션) |
| **슬래시 명령어** | /status, /sessions, /tools, /cancel, /approve |
| **DM 정책** | Pairing/Allowlist/Open/Disabled 모드 |
| **시스템 알림** | notify_chat_id로 승인 요청/에러 알림 |

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    Telegram                                  │
│  ┌─────────────────┐  ┌─────────────────┐                  │
│  │  개인 채팅       │  │  그룹 채팅       │                  │
│  │  "요약해줘"     │  │  @Cratos 안녕   │                  │
│  └────────┬────────┘  └────────┬────────┘                  │
└───────────│─────────────────────│──────────────────────────┘
            │ Telegram Bot API    │
            └──────────┬──────────┘
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                    Cratos Server                            │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                  TelegramAdapter                         ││
│  │  ┌───────────┐  ┌───────────┐  ┌───────────────────┐   ││
│  │  │ teloxide  │  │ Message   │  │ Security          │   ││
│  │  │ Bot       │  │ Handler   │  │ (마스킹/정제)     │   ││
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

### 1. BotFather로 봇 생성

1. Telegram에서 [@BotFather](https://t.me/BotFather) 검색
2. `/newbot` 명령어 입력
3. 봇 이름 입력 (예: "Cratos Assistant")
4. 봇 사용자명 입력 (예: `cratos_assistant_bot`, `_bot`으로 끝나야 함)
5. 토큰 복사 (형식: `123456789:ABCdefGHIjklMNOpqrsTUVwxyz`)

```
BotFather: Done! Congratulations on your new bot.

Use this token to access the HTTP API:
123456789:ABCdefGHIjklMNOpqrsTUVwxyz

Keep your token secure and store it safely.
```

### 2. 봇 설정 (선택)

BotFather에서 추가 설정:

```
/setdescription - 봇 소개 설정
/setabouttext - 봇 정보 설정
/setuserpic - 봇 프로필 사진 설정
/setcommands - 명령어 목록 설정
```

명령어 목록 예시:
```
help - 도움말 보기
status - 상태 확인
cancel - 현재 작업 취소
```

### 3. 환경 변수 설정

```bash
# .env
TELEGRAM_BOT_TOKEN=123456789:ABCdefGHIjklMNOpqrsTUVwxyz

# 선택 옵션
TELEGRAM_ALLOWED_USERS=123456789,987654321  # 허용 사용자 ID (빈 값 = 모두 허용)
TELEGRAM_ALLOWED_GROUPS=-100123456789       # 허용 그룹 ID (빈 값 = 모두 허용)
TELEGRAM_GROUPS_MENTION_ONLY=true           # 그룹에서 @멘션/답글만 응답
```

### 4. 사용자/그룹 ID 확인 방법

**사용자 ID 확인:**
1. [@userinfobot](https://t.me/userinfobot) 에게 메시지 전송
2. 또는 [@getmyid_bot](https://t.me/getmyid_bot) 사용

**그룹 ID 확인:**
1. 봇을 그룹에 추가
2. 그룹에서 아무 메시지 전송
3. 브라우저에서 확인:
   ```
   https://api.telegram.org/bot<TOKEN>/getUpdates
   ```
4. `chat.id` 값 확인 (그룹은 음수)

## 사용 방법

### 개인 채팅에서

@멘션 없이 바로 대화:

```
사용자: 안녕!
Cratos: 안녕하세요! 무엇을 도와드릴까요?

사용자: 피보나치 함수 만들어줘
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

### 그룹에서

`TELEGRAM_GROUPS_MENTION_ONLY=true` (기본값)일 때:

```
[일반 메시지 - 무시됨]
사용자A: 점심 뭐 먹지?

[@멘션으로 호출]
사용자B: @cratos_bot 오늘 날씨 어때?
Cratos: 현재 서울 날씨는...

[답글로 호출]
사용자A: (Cratos 메시지에 답글) 내일은?
Cratos: 내일 날씨 예보는...
```

### 답글 컨텍스트

이전 대화에 답글 달면 컨텍스트 유지:

```
사용자: 파이썬으로 리스트 정렬하는 방법 알려줘
Cratos: sorted() 함수나 .sort() 메서드를 사용...

[위 메시지에 답글]
사용자: 내림차순은?
Cratos: reverse=True 파라미터를 사용하면 됩니다...
```

### 첨부 파일

이미지나 문서 첨부 가능:

```
사용자: [이미지 첨부] 이 코드 리뷰해줘
Cratos: 이미지를 분석했습니다. 코드에서 다음 사항을...

사용자: [PDF 첨부] 이 문서 요약해줘
Cratos: 문서 내용을 요약하면...
```

## 설정 옵션

### TelegramConfig

```rust
pub struct TelegramConfig {
    /// 봇 토큰 (필수)
    pub bot_token: String,

    /// 허용된 사용자 ID 목록 (빈 값 = 모든 사용자 허용)
    pub allowed_users: Vec<i64>,

    /// 허용된 그룹 ID 목록 (빈 값 = 모든 그룹 허용)
    pub allowed_groups: Vec<i64>,

    /// 그룹에서 @멘션/답글에만 응답 (기본: true)
    pub groups_mention_only: bool,

    /// DM 보안 정책 (Pairing/Allowlist/Open/Disabled)
    pub dm_policy: DmPolicy,

    /// 시스템 알림용 채팅 ID (승인 요청, 에러 등)
    pub notify_chat_id: Option<i64>,
}
```

### DmPolicy

```rust
pub enum DmPolicy {
    /// 알 수 없는 사용자에게 페어링 코드 요구
    Pairing,
    /// allowed_users 목록에 있는 사용자만 DM 허용
    Allowlist,
    /// 모든 사용자 DM 허용 (최소 보안)
    Open,
    /// DM 처리 완전 비활성화
    Disabled,
}
```

### 프로그래매틱 설정

```rust
use cratos_channels::telegram::{TelegramAdapter, TelegramConfig};

// 기본 설정
let config = TelegramConfig::new("YOUR_BOT_TOKEN");

// 상세 설정
let config = TelegramConfig::new("YOUR_BOT_TOKEN")
    .with_allowed_users(vec![123456789, 987654321])
    .with_allowed_groups(vec![-100123456789])
    .with_groups_mention_only(true);

let adapter = TelegramAdapter::new(config);

// 또는 환경 변수에서 생성
let adapter = TelegramAdapter::from_env()?;
```

### 환경 변수

| 변수 | 필수 | 기본값 | 설명 |
|------|------|--------|------|
| `TELEGRAM_BOT_TOKEN` | ✅ | - | 봇 토큰 |
| `TELEGRAM_ALLOWED_USERS` | ❌ | 빈 값 | 쉼표로 구분된 사용자 ID |
| `TELEGRAM_ALLOWED_GROUPS` | ❌ | 빈 값 | 쉼표로 구분된 그룹 ID |
| `TELEGRAM_GROUPS_MENTION_ONLY` | ❌ | true | false면 그룹의 모든 메시지에 응답 |
| `TELEGRAM_DM_POLICY` | ❌ | `allowlist` | DM 정책 (pairing/allowlist/open/disabled) |
| `TELEGRAM_NOTIFY_CHAT_ID` | ❌ | - | 시스템 알림 수신 채팅 ID |

## 슬래시 명령어

Telegram에서 사용 가능한 슬래시 명령어:

| 명령어 | 설명 |
|--------|------|
| `/status` | 시스템 상태 (활성 실행 수, 등록 도구 수, 업타임) |
| `/sessions` | 활성 개발 세션 목록 (DevSessionMonitor 필요) |
| `/tools` | 등록된 도구 목록 |
| `/cancel <execution_id>` | 실행 중인 작업 취소 |
| `/approve <request_id>` | 승인 대기 중인 도구 실행 승인 |

BotFather에서 명령어 등록:
```
status - 시스템 상태 확인
sessions - 활성 개발 세션 목록
tools - 등록된 도구 목록
cancel - 실행 취소
approve - 도구 실행 승인
```

## 보안

### 민감 정보 마스킹

로그에 민감 정보가 노출되지 않도록 자동 마스킹:

```rust
// 다음 패턴 포함 시 [REDACTED] 처리
const SENSITIVE_PATTERNS: &[&str] = &[
    "password", "passwd", "secret", "token",
    "api_key", "apikey", "api-key", "bearer",
    "authorization", "credential", "private",
    "ssh", "-----begin"
];

// 예시
// 입력: "my password is secret123"
// 로그: "[REDACTED - potentially sensitive content]"
```

### 긴 메시지 자르기

50자 이상 메시지는 로그에서 자동으로 잘림:

```rust
const MAX_LOG_TEXT_LENGTH: usize = 50;

// 입력: "This is a very long message that..."
// 로그: "This is a very long message that co...[truncated]"
```

### 에러 메시지 정제

사용자에게 내부 에러 노출 방지:

```rust
// 내부: "Invalid token: sk-abc123..."
// 사용자: "An authentication error occurred. Please check your configuration."

// 내부: "Connection timeout to database"
// 사용자: "A network error occurred. Please try again later."

// 내부: "SQL error: SELECT * FROM users"
// 사용자: "A database error occurred. Please try again later."
```

### 권한 제한

```bash
# 특정 사용자만 허용
TELEGRAM_ALLOWED_USERS=123456789

# 특정 그룹만 허용
TELEGRAM_ALLOWED_GROUPS=-100123456789,-100987654321

# 조합 사용
TELEGRAM_ALLOWED_USERS=123456789
TELEGRAM_ALLOWED_GROUPS=-100123456789
```

## API 레퍼런스

### TelegramAdapter

```rust
impl TelegramAdapter {
    /// 새 어댑터 생성
    pub fn new(config: TelegramConfig) -> Self;

    /// 환경 변수에서 생성
    pub fn from_env() -> Result<Self>;

    /// 기본 teloxide Bot 인스턴스 반환
    pub fn bot(&self) -> &Bot;

    /// 사용자 허용 여부 확인
    pub fn is_user_allowed(&self, user_id: i64) -> bool;

    /// 그룹 허용 여부 확인
    pub fn is_group_allowed(&self, chat_id: i64) -> bool;

    /// Telegram 메시지를 정규화된 메시지로 변환
    pub fn normalize_message(
        &self,
        msg: &TelegramMessage,
        bot_username: &str
    ) -> Option<NormalizedMessage>;

    /// 봇 실행
    pub async fn run(
        self: Arc<Self>,
        orchestrator: Arc<Orchestrator>,
        dev_monitor: Option<Arc<DevSessionMonitor>>,
    ) -> Result<()>;
}
```

### TelegramConfig

```rust
impl TelegramConfig {
    /// 환경 변수에서 생성
    pub fn from_env() -> Result<Self>;

    /// 토큰으로 생성
    pub fn new(bot_token: impl Into<String>) -> Self;

    /// 허용 사용자 설정 (빌더 패턴)
    pub fn with_allowed_users(self, users: Vec<i64>) -> Self;

    /// 허용 그룹 설정 (빌더 패턴)
    pub fn with_allowed_groups(self, groups: Vec<i64>) -> Self;

    /// 그룹 멘션 전용 모드 설정 (빌더 패턴)
    pub fn with_groups_mention_only(self, enabled: bool) -> Self;

    /// DM 정책 설정 (빌더 패턴)
    pub fn with_dm_policy(self, policy: DmPolicy) -> Self;

    /// 알림 채팅 ID 설정 (빌더 패턴)
    pub fn with_notify_chat_id(self, chat_id: i64) -> Self;
}
```

### ChannelAdapter 구현

```rust
impl ChannelAdapter for TelegramAdapter {
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

    /// 타이핑 표시
    async fn send_typing(&self, channel_id: &str) -> Result<()>;
}
```

### OutgoingMessage 옵션

```rust
pub struct OutgoingMessage {
    /// 메시지 텍스트
    pub text: String,
    /// 마크다운 파싱 여부
    pub parse_markdown: bool,
    /// 답글 대상 메시지 ID
    pub reply_to: Option<String>,
    /// 인라인 키보드 버튼
    pub buttons: Vec<MessageButton>,
}
```

### 인라인 키보드 사용

```rust
use cratos_channels::message::{MessageButton, OutgoingMessage};

let message = OutgoingMessage {
    text: "선택하세요:".to_string(),
    parse_markdown: false,
    reply_to: None,
    buttons: vec![
        MessageButton::callback("예", "approve:yes"),
        MessageButton::callback("아니오", "approve:no"),
        MessageButton::url("문서 보기", "https://docs.example.com"),
    ],
};

adapter.send_message("123456789", message).await?;
```

## 문제 해결

### 봇이 응답하지 않음

1. **토큰 확인**
   ```bash
   # 토큰 형식 확인 (숫자:영문숫자)
   echo $TELEGRAM_BOT_TOKEN
   # 올바른 형식: 123456789:ABCdefGHIjklMNOpqrsTUVwxyz
   ```

2. **권한 설정 확인**
   ```bash
   # 허용 사용자/그룹에 포함되어 있는지 확인
   echo $TELEGRAM_ALLOWED_USERS
   echo $TELEGRAM_ALLOWED_GROUPS
   ```

3. **그룹 멘션 모드 확인**
   ```bash
   # true면 @멘션이나 답글 필요
   echo $TELEGRAM_GROUPS_MENTION_ONLY
   ```

4. **봇 프라이버시 모드**
   - BotFather에서 `/setprivacy` → `Disable`
   - 그룹의 모든 메시지 수신 가능

### 그룹에서 메시지를 못 받음

1. 봇이 그룹 관리자인지 확인
2. 또는 BotFather에서 `/setprivacy` → `Disable`

### "Unauthorized" 에러

```bash
# 토큰이 만료되었거나 잘못됨
# BotFather에서 /token으로 새 토큰 발급
```

### 마크다운 렌더링

Cratos는 MarkdownV2 대신 **HTML 파싱 모드**를 사용합니다 (특수 문자 이스케이프 문제 방지):

```rust
// 응답은 markdown_to_html()로 변환 후 ParseMode::Html로 전송
// bold, italic, code, codeblock, strikethrough, link 변환 지원
bot.send_message(chat_id, &markdown_to_html(&response_text))
    .parse_mode(ParseMode::Html)
    .await;
```

HTML 파싱 실패 시 일반 텍스트로 자동 폴백:
```rust
// HTML 파싱 실패 시 자동으로 일반 텍스트로 재전송
if send_result.is_err() {
    bot.send_message(chat_id, &response_text).await;
}
```

### Rate Limit (429 에러)

Telegram Bot API 제한:
- 초당 30개 메시지 (전체)
- 같은 그룹에 분당 20개

```
⚠️ Too Many Requests: retry after 30
→ 30초 후 자동 재시도
```

## 첨부 파일 처리

### 지원 타입

| 타입 | AttachmentType | 설명 |
|------|----------------|------|
| 사진 | `Image` | JPEG 형식, 가장 큰 해상도 선택 |
| 문서 | `Document` | 모든 파일 형식 |

### 첨부 파일 정보

```rust
pub struct Attachment {
    /// 첨부 타입
    pub attachment_type: AttachmentType,
    /// 파일 이름 (문서만)
    pub file_name: Option<String>,
    /// MIME 타입
    pub mime_type: Option<String>,
    /// 파일 크기 (바이트)
    pub file_size: Option<u64>,
    /// 다운로드 URL
    pub url: Option<String>,
    /// Telegram 파일 ID
    pub file_id: Option<String>,
}
```

## 향후 계획

1. **v1.1**: 콜백 쿼리 핸들링 (버튼 클릭 처리)
2. **v1.2**: 파일 업로드/다운로드 지원
3. **v1.3**: 인라인 모드 지원 (`@bot query`)
4. **v2.0**: 웹훅 모드 지원 (폴링 대신)
