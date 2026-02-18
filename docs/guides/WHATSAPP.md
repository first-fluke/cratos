# WhatsApp 연동 가이드

## 개요

Cratos를 WhatsApp과 연동하여 메신저에서 AI 어시스턴트를 사용할 수 있습니다. 두 가지 연동 방식을 제공합니다.

### 연동 옵션 비교

| 기능               | Baileys (비공식)               | Business API (공식)   |
| ------------------ | ------------------------------ | --------------------- |
| **비용**           | 무료                           | 유료 (메시지당 과금)  |
| **계정 요구**      | 일반 WhatsApp 계정             | Meta Business 계정    |
| **설정 난이도**    | 쉬움 (QR 스캔)                 | 복잡 (Meta 승인 필요) |
| **안정성**         | 불안정 (업데이트 시 중단 가능) | 안정적                |
| **계정 차단 위험** | ⚠️ 높음                         | 없음                  |
| **ToS 준수**       | ❌ 위반                         | ✅ 준수                |
| **프로덕션 권장**  | ❌                              | ✅                     |

### 주요 기능

| 기능              | Baileys | Business API |
| ----------------- | ------- | ------------ |
| **텍스트 메시지** | ✅       | ✅            |
| **타이핑 표시**   | ✅       | ❌            |
| **읽음 표시**     | ❌       | ✅            |
| **그룹 메시지**   | ✅       | ❌            |
| **번호 제한**     | ✅       | ✅            |
| **메시지 수정**   | ❌       | ❌            |
| **메시지 삭제**   | ❌       | ❌            |

## 아키텍처

### Option 1: Baileys Bridge (비공식)

```
┌─────────────────────────────────────────────────────────────┐
│                    WhatsApp Mobile App                       │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  📱 QR 코드 스캔으로 연결                                 ││
│  └─────────────────────────────────────────────────────────┘│
└──────────────────────────┬──────────────────────────────────┘
                           │ WhatsApp Web Protocol (unofficial)
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Node.js Baileys Bridge Server                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  Baileys    │  │  Session    │  │  REST API           │ │
│  │  Library    │  │  Manager    │  │  (localhost:3001)   │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└──────────────────────────┬──────────────────────────────────┘
                           │ HTTP/Webhook
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    Cratos Server                             │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                  WhatsAppAdapter                         ││
│  │  ┌───────────┐  ┌───────────┐  ┌───────────────────┐   ││
│  │  │ reqwest   │  │ Webhook   │  │ Message           │   ││
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

### Option 2: Business Cloud API (공식)

```
┌─────────────────────────────────────────────────────────────┐
│                    WhatsApp Users                            │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  📱 비즈니스 번호로 메시지 전송                           ││
│  └─────────────────────────────────────────────────────────┘│
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Meta WhatsApp Cloud API                         │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  graph.facebook.com/v18.0/{phone_number_id}/messages    ││
│  └─────────────────────────────────────────────────────────┘│
└──────────────────────────┬──────────────────────────────────┘
                           │ Webhook (HTTPS)
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    Cratos Server                             │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              WhatsAppBusinessAdapter                     ││
│  │  ┌───────────┐  ┌───────────┐  ┌───────────────────┐   ││
│  │  │ reqwest   │  │ Webhook   │  │ Message           │   ││
│  │  │ Client    │  │ Verify    │  │ Normalizer        │   ││
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

### Option 1: Baileys Bridge (비공식)

> ⚠️ **경고**: Baileys는 비공식 역공학 라이브러리입니다.
> - 계정 **영구 차단** 위험이 있습니다
> - Meta 서비스 약관을 위반합니다
> - WhatsApp 업데이트 시 중단될 수 있습니다
> - **중요한 계정으로 사용하지 마세요**
> - 프로덕션/비즈니스 용도로는 Business API를 사용하세요

#### 1. Baileys Bridge 서버 설정

```bash
# baileys-bridge 디렉토리 생성
mkdir baileys-bridge && cd baileys-bridge

# package.json 생성
npm init -y

# 의존성 설치
npm install @whiskeysockets/baileys express qrcode-terminal
```

#### 2. Bridge 서버 코드 (예시)

```javascript
// bridge.js
const { default: makeWASocket, useMultiFileAuthState } = require('@whiskeysockets/baileys');
const express = require('express');
const qrcode = require('qrcode-terminal');

const app = express();
app.use(express.json());

let sock = null;
let qrCode = null;

async function connectWhatsApp() {
    const { state, saveCreds } = await useMultiFileAuthState('auth_info');

    sock = makeWASocket({ auth: state });

    sock.ev.on('creds.update', saveCreds);

    sock.ev.on('connection.update', (update) => {
        const { qr, connection } = update;
        if (qr) {
            qrCode = qr;
            qrcode.generate(qr, { small: true });
        }
        if (connection === 'close') {
            setTimeout(connectWhatsApp, 5000);
        }
    });

    sock.ev.on('messages.upsert', async ({ messages }) => {
        // Cratos 웹훅으로 전달
        for (const msg of messages) {
            if (!msg.key.fromMe && msg.message?.conversation) {
                await fetch('http://localhost:19527/webhook/whatsapp', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        id: msg.key.id,
                        from: msg.key.remoteJid,
                        participant: msg.key.participant,
                        text: msg.message.conversation,
                        timestamp: msg.messageTimestamp,
                        isGroup: msg.key.remoteJid.endsWith('@g.us')
                    })
                });
            }
        }
    });
}

app.get('/status', (req, res) => {
    res.json({
        status: sock?.user ? 'connected' : (qrCode ? 'waiting_scan' : 'disconnected'),
        qr: qrCode,
        connected: !!sock?.user
    });
});

app.post('/connect', async (req, res) => {
    if (!sock) await connectWhatsApp();
    res.json({ status: 'connecting', qr: qrCode });
});

app.post('/send', async (req, res) => {
    const { to, message, quotedId } = req.body;
    try {
        const result = await sock.sendMessage(to, { text: message }, { quoted: quotedId });
        res.json({ success: true, messageId: result.key.id });
    } catch (e) {
        res.json({ success: false, error: e.message });
    }
});

app.post('/typing', async (req, res) => {
    const { to } = req.body;
    await sock.sendPresenceUpdate('composing', to);
    res.json({ success: true });
});

app.listen(3001, () => console.log('Bridge running on :3001'));
connectWhatsApp();
```

#### 3. Bridge 서버 실행

```bash
node bridge.js
```

#### 4. 환경 변수 설정

```bash
# .env
WHATSAPP_BRIDGE_URL=http://localhost:3001

# 선택 옵션
WHATSAPP_ALLOWED_NUMBERS=+821012345678,+821098765432  # 허용 번호 (빈 값 = 모든 번호)
WHATSAPP_TIMEOUT=30                                    # 요청 타임아웃 (초)
```

#### 5. QR 코드 스캔

1. Bridge 서버 실행 시 터미널에 QR 코드 표시
2. WhatsApp 앱 → 연결된 기기 → 기기 연결
3. QR 코드 스캔
4. 연결 완료!

---

### Option 2: Business Cloud API (공식)

#### 1. Meta Business 계정 설정

1. [Meta Business Suite](https://business.facebook.com/) 접속
2. 비즈니스 계정 생성 (없는 경우)
3. [Meta for Developers](https://developers.facebook.com/) 접속
4. "내 앱" → "앱 만들기" → "비즈니스" 선택

#### 2. WhatsApp Business API 설정

1. 앱 대시보드 → "제품 추가" → "WhatsApp" 선택
2. "시작하기" 클릭
3. WhatsApp 비즈니스 계정 연결 또는 생성
4. 테스트 전화번호 받기 (또는 자체 번호 등록)

#### 3. API 자격 증명 획득

앱 대시보드에서 다음 정보 확인:

- **Access Token**: 임시 또는 영구 토큰
- **Phone Number ID**: 봇의 전화번호 ID
- **Business Account ID**: 비즈니스 계정 ID

#### 4. Webhook 설정

1. 앱 대시보드 → WhatsApp → 구성
2. Webhook URL 입력: `https://your-domain.com/webhook/whatsapp-business`
3. Verify Token 설정 (사용자 정의)
4. 구독 필드 선택:
   - `messages` (필수)
   - `message_deliveries` (선택)
   - `message_reads` (선택)

#### 5. 환경 변수 설정

```bash
# .env (필수)
WHATSAPP_ACCESS_TOKEN=EAAxxxxxxxxx...
WHATSAPP_PHONE_NUMBER_ID=123456789012345
WHATSAPP_BUSINESS_ACCOUNT_ID=123456789012345

# 선택 옵션
WHATSAPP_WEBHOOK_VERIFY_TOKEN=cratos_webhook_verify  # Webhook 검증 토큰
WHATSAPP_ALLOWED_NUMBERS=+821012345678               # 허용 번호 (빈 값 = 모든 번호)
WHATSAPP_API_VERSION=v18.0                           # API 버전
```

## 사용 방법

### 1:1 대화

```
사용자: 안녕하세요!
Cratos: 안녕하세요! 무엇을 도와드릴까요?

사용자: 파이썬으로 피보나치 함수 만들어줘
Cratos: def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)
```

### 그룹 메시지 (Baileys만 지원)

그룹에서 봇 번호를 멘션하거나 그룹 설정에 따라 응답:

```
[그룹: 개발팀]
사용자: 오늘 배포 일정 알려줘
Cratos: 오늘 예정된 배포는...
```

## 설정 옵션

### WhatsAppConfig (Baileys)

```rust
pub struct WhatsAppConfig {
    /// Bridge 서버 URL (기본: http://localhost:3001)
    pub bridge_url: String,

    /// 허용된 전화번호 목록 (빈 값 = 모든 번호 허용)
    pub allowed_numbers: Vec<String>,

    /// 요청 타임아웃 (초, 기본: 30)
    pub timeout_secs: u64,
}
```

### WhatsAppBusinessConfig (Business API)

```rust
pub struct WhatsAppBusinessConfig {
    /// Access Token (필수, Meta Business Suite에서 발급)
    pub access_token: String,

    /// Phone Number ID (필수, 봇의 전화번호 ID)
    pub phone_number_id: String,

    /// Business Account ID (필수)
    pub business_account_id: String,

    /// Webhook 검증 토큰 (기본: cratos_webhook_verify)
    pub webhook_verify_token: String,

    /// 허용된 전화번호 목록 (빈 값 = 모든 번호 허용)
    pub allowed_numbers: Vec<String>,

    /// API 버전 (기본: v18.0)
    pub api_version: String,
}
```

### 환경 변수

#### Baileys

| 변수                       | 필수 | 기본값                  | 설명                    |
| -------------------------- | ---- | ----------------------- | ----------------------- |
| `WHATSAPP_BRIDGE_URL`      | ❌    | `http://localhost:3001` | Bridge 서버 URL         |
| `WHATSAPP_ALLOWED_NUMBERS` | ❌    | 빈 값                   | 쉼표로 구분된 허용 번호 |
| `WHATSAPP_TIMEOUT`         | ❌    | 30                      | 요청 타임아웃 (초)      |

#### Business API

| 변수                            | 필수 | 기본값                  | 설명                    |
| ------------------------------- | ---- | ----------------------- | ----------------------- |
| `WHATSAPP_ACCESS_TOKEN`         | ✅    | -                       | Meta Access Token       |
| `WHATSAPP_PHONE_NUMBER_ID`      | ✅    | -                       | 전화번호 ID             |
| `WHATSAPP_BUSINESS_ACCOUNT_ID`  | ✅    | -                       | 비즈니스 계정 ID        |
| `WHATSAPP_WEBHOOK_VERIFY_TOKEN` | ❌    | `cratos_webhook_verify` | Webhook 검증 토큰       |
| `WHATSAPP_ALLOWED_NUMBERS`      | ❌    | 빈 값                   | 쉼표로 구분된 허용 번호 |
| `WHATSAPP_API_VERSION`          | ❌    | `v18.0`                 | Graph API 버전          |

## 보안

### 민감 정보 마스킹

로그에 민감 정보가 노출되지 않도록 자동 마스킹:

```rust
// 다음 패턴 포함 시 [REDACTED] 처리
const SENSITIVE_PATTERNS: &[&str] = &[
    "password", "secret", "token", "api_key",
    "bearer", "credential", "private"
];
```

### 번호 제한

특정 번호만 허용하여 무단 접근 방지:

```bash
# 허용 번호 설정 (국가 코드 포함)
WHATSAPP_ALLOWED_NUMBERS=+821012345678,+821098765432
```

번호 비교 시 자동 정규화:
- `+82-10-1234-5678` → `821012345678`
- `010-1234-5678` → `1012345678`

### Webhook 검증 (Business API)

Meta에서 Webhook 설정 시 검증 요청:

```
GET /webhook/whatsapp-business?hub.mode=subscribe&hub.verify_token=YOUR_TOKEN&hub.challenge=CHALLENGE
```

Cratos가 자동으로 검증 처리:

```rust
pub fn verify_webhook(&self, mode: &str, token: &str, challenge: &str) -> Option<String> {
    if mode == "subscribe" && token == self.config.webhook_verify_token {
        Some(challenge.to_string())
    } else {
        None
    }
}
```

### Access Token 보안 (Business API)

- 환경 변수로 관리, 코드에 하드코딩 금지
- 영구 토큰 사용 시 정기적 갱신 권장
- 토큰 노출 시 즉시 재발급

## API 레퍼런스

### WhatsAppAdapter (Baileys)

```rust
impl WhatsAppAdapter {
    /// 새 어댑터 생성
    pub fn new(config: WhatsAppConfig) -> Self;

    /// 환경 변수에서 생성
    pub fn from_env() -> Result<Self>;

    /// 연결 상태 확인
    pub async fn status(&self) -> Result<ConnectionStatus>;

    /// 연결 시작 (QR 코드 반환 가능)
    pub async fn connect(&self) -> Result<WhatsAppConnection>;

    /// 연결 해제
    pub async fn disconnect(&self) -> Result<()>;

    /// 연결 여부 확인
    pub fn is_connected(&self) -> bool;

    /// 번호 허용 여부 확인
    pub fn is_number_allowed(&self, number: &str) -> bool;

    /// Webhook 메시지 처리
    pub async fn handle_webhook(
        &self,
        orchestrator: Arc<Orchestrator>,
        msg: WhatsAppWebhookMessage,
    ) -> Result<()>;
}
```

### WhatsAppBusinessAdapter (Business API)

```rust
impl WhatsAppBusinessAdapter {
    /// 새 어댑터 생성
    pub fn new(config: WhatsAppBusinessConfig) -> Self;

    /// 환경 변수에서 생성
    pub fn from_env() -> Result<Self>;

    /// Webhook 검증
    pub fn verify_webhook(&self, mode: &str, token: &str, challenge: &str) -> Option<String>;

    /// 번호 허용 여부 확인
    pub fn is_number_allowed(&self, number: &str) -> bool;

    /// Webhook 메시지 추출
    pub fn extract_messages(&self, webhook: &WhatsAppBusinessWebhook) -> Vec<(String, WebhookMessage)>;

    /// Webhook 처리
    pub async fn handle_webhook(
        &self,
        orchestrator: Arc<Orchestrator>,
        webhook: WhatsAppBusinessWebhook,
    ) -> Result<()>;

    /// 메시지 읽음 처리
    pub async fn mark_as_read(&self, message_id: &str) -> Result<()>;
}
```

### ChannelAdapter 구현

```rust
impl ChannelAdapter for WhatsAppAdapter {
    /// 메시지 전송
    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String>;

    /// 메시지 수정 (미지원)
    async fn edit_message(&self, channel_id: &str, message_id: &str, message: OutgoingMessage) -> Result<()>;

    /// 메시지 삭제 (미지원)
    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()>;

    /// 타이핑 표시
    async fn send_typing(&self, channel_id: &str) -> Result<()>;
}
```

## 제한 사항

### 기능 제한 비교

| 기능          | Baileys | Business API | 비고            |
| ------------- | ------- | ------------ | --------------- |
| 메시지 전송   | ✅       | ✅            |                 |
| 메시지 수정   | ❌       | ❌            | WhatsApp 미지원 |
| 메시지 삭제   | ❌       | ❌            | 구현 복잡       |
| 타이핑 표시   | ✅       | ❌            | API 미제공      |
| 읽음 표시     | ❌       | ✅            |                 |
| 그룹 메시지   | ✅       | ❌            | 별도 권한 필요  |
| 미디어 전송   | ❌       | ❌            | 향후 지원 예정  |
| 템플릿 메시지 | ❌       | ❌            | 향후 지원 예정  |

### 메시지 길이 제한

```rust
// 4096자 초과 시 자동 분할
if response_text.len() > 4096 {
    for chunk in response_text.as_bytes().chunks(4096) {
        // 청크 단위로 전송
    }
}
```

### Business API 제한

- **24시간 윈도우**: 사용자가 먼저 메시지를 보낸 후 24시간 내에만 자유롭게 응답 가능
- **템플릿 메시지**: 24시간 이후에는 사전 승인된 템플릿만 사용 가능
- **요금**: 메시지당 과금 (국가별 상이)

## 문제 해결

### Baileys 관련

#### Bridge 서버 연결 실패

```bash
# Bridge 서버 상태 확인
curl http://localhost:3001/status

# 응답 예시
{"status":"connected","qr":null,"connected":true}
```

#### QR 코드가 계속 새로 생성됨

1. `auth_info` 디렉토리 권한 확인
2. 기존 세션 파일 삭제 후 재시도
3. WhatsApp 앱에서 기존 웹 세션 해제

#### 계정 차단 경고

WhatsApp이 의심스러운 활동 감지 시:
- 새 기기에서 너무 빈번한 메시지 전송
- 자동화된 메시지 패턴
- 다수의 알 수 없는 번호에 메시지

**대응 방법**:
1. 즉시 봇 중지
2. 몇 시간 대기
3. 메시지 빈도 감소
4. Business API로 전환 고려

### Business API 관련

#### "Invalid Access Token" 에러

```bash
# 토큰 유효성 확인
curl "https://graph.facebook.com/v18.0/me?access_token=YOUR_TOKEN"
```

토큰 만료 시:
1. Meta Business Suite에서 새 토큰 발급
2. 영구 토큰으로 전환 고려

#### Webhook 검증 실패

1. `WHATSAPP_WEBHOOK_VERIFY_TOKEN`이 Meta 대시보드 설정과 일치하는지 확인
2. HTTPS 엔드포인트인지 확인 (필수)
3. 방화벽/프록시 설정 확인

#### API 에러 코드

| 코드   | 의미                 | 해결 방법          |
| ------ | -------------------- | ------------------ |
| 190    | Invalid Access Token | 토큰 재발급        |
| 368    | Rate Limit           | 요청 빈도 감소     |
| 131030 | 24시간 윈도우 초과   | 템플릿 메시지 사용 |
| 131047 | 수신자가 차단        | 다른 번호 시도     |

## 향후 계획

1. **v1.1**: 미디어 메시지 지원 (이미지, 문서)
2. **v1.2**: 템플릿 메시지 지원 (Business API)
3. **v1.3**: 버튼/리스트 인터랙티브 메시지
4. **v2.0**: WhatsApp Flows 연동
