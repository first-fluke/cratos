# WhatsApp Integration Guide

## Overview

Integrate Cratos with WhatsApp to use the AI assistant in your messaging app. Two integration options are available.

### Integration Options Comparison

| Feature | Baileys (Unofficial) | Business API (Official) |
|---------|---------------------|-------------------------|
| **Cost** | Free | Paid (per-message pricing) |
| **Account Required** | Regular WhatsApp account | Meta Business account |
| **Setup Difficulty** | Easy (QR scan) | Complex (Meta approval required) |
| **Stability** | Unstable (may break on updates) | Stable |
| **Account Ban Risk** | High | None |
| **ToS Compliance** | Violates | Compliant |
| **Production Recommended** | No | Yes |

### Key Features

| Feature | Baileys | Business API |
|---------|---------|--------------|
| **Text Messages** | Yes | Yes |
| **Typing Indicator** | Yes | No |
| **Read Receipts** | No | Yes |
| **Group Messages** | Yes | No |
| **Number Filtering** | Yes | Yes |
| **Message Editing** | No | No |
| **Message Deletion** | No | No |

## Architecture

### Option 1: Baileys Bridge (Unofficial)

```
┌─────────────────────────────────────────────────────────────┐
│                    WhatsApp Mobile App                       │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  Scan QR code to connect                                 ││
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
│  │         (LLM Processing → Tool Execution → Response)     ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Option 2: Business Cloud API (Official)

```
┌─────────────────────────────────────────────────────────────┐
│                    WhatsApp Users                            │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  Send messages to business number                        ││
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
│  │         (LLM Processing → Tool Execution → Response)     ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## Setup

### Option 1: Baileys Bridge (Unofficial)

> **Warning**: Baileys is an unofficial reverse-engineered library.
> - Risk of **permanent account ban**
> - Violates Meta Terms of Service
> - May break with WhatsApp updates
> - **Do NOT use with important accounts**
> - For production/business use, use the Business API instead

#### 1. Set Up Baileys Bridge Server

```bash
# Create baileys-bridge directory
mkdir baileys-bridge && cd baileys-bridge

# Initialize package.json
npm init -y

# Install dependencies
npm install @whiskeysockets/baileys express qrcode-terminal
```

#### 2. Bridge Server Code (Example)

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
        // Forward to Cratos webhook
        for (const msg of messages) {
            if (!msg.key.fromMe && msg.message?.conversation) {
                await fetch('http://localhost:8080/webhook/whatsapp', {
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

#### 3. Run Bridge Server

```bash
node bridge.js
```

#### 4. Set Environment Variables

```bash
# .env
WHATSAPP_BRIDGE_URL=http://localhost:3001

# Optional
WHATSAPP_ALLOWED_NUMBERS=+821012345678,+821098765432  # Allowed numbers (empty = all)
WHATSAPP_TIMEOUT=30                                    # Request timeout (seconds)
```

#### 5. Scan QR Code

1. QR code displays in terminal when bridge server starts
2. WhatsApp app -> Linked Devices -> Link a Device
3. Scan the QR code
4. Connected!

---

### Option 2: Business Cloud API (Official)

#### 1. Set Up Meta Business Account

1. Go to [Meta Business Suite](https://business.facebook.com/)
2. Create a business account (if you don't have one)
3. Go to [Meta for Developers](https://developers.facebook.com/)
4. "My Apps" -> "Create App" -> Select "Business"

#### 2. Configure WhatsApp Business API

1. App Dashboard -> "Add Products" -> Select "WhatsApp"
2. Click "Get Started"
3. Connect or create WhatsApp Business account
4. Get test phone number (or register your own number)

#### 3. Get API Credentials

Find these in the App Dashboard:

- **Access Token**: Temporary or permanent token
- **Phone Number ID**: Your bot's phone number ID
- **Business Account ID**: Your business account ID

#### 4. Configure Webhook

1. App Dashboard -> WhatsApp -> Configuration
2. Enter Webhook URL: `https://your-domain.com/webhook/whatsapp-business`
3. Set Verify Token (custom value)
4. Select subscription fields:
   - `messages` (required)
   - `message_deliveries` (optional)
   - `message_reads` (optional)

#### 5. Set Environment Variables

```bash
# .env (required)
WHATSAPP_ACCESS_TOKEN=EAAxxxxxxxxx...
WHATSAPP_PHONE_NUMBER_ID=123456789012345
WHATSAPP_BUSINESS_ACCOUNT_ID=123456789012345

# Optional
WHATSAPP_WEBHOOK_VERIFY_TOKEN=cratos_webhook_verify  # Webhook verification token
WHATSAPP_ALLOWED_NUMBERS=+821012345678               # Allowed numbers (empty = all)
WHATSAPP_API_VERSION=v18.0                           # API version
```

## Usage

### 1:1 Conversations

```
User: Hello!
Cratos: Hello! How can I help you?

User: Create a fibonacci function in Python
Cratos: def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)
```

### Group Messages (Baileys Only)

Mention the bot number in groups or respond based on group settings:

```
[Group: Dev Team]
User: What's the deployment schedule today?
Cratos: Today's scheduled deployments are...
```

## Configuration Options

### WhatsAppConfig (Baileys)

```rust
pub struct WhatsAppConfig {
    /// Bridge server URL (default: http://localhost:3001)
    pub bridge_url: String,

    /// Allowed phone numbers (empty = allow all)
    pub allowed_numbers: Vec<String>,

    /// Request timeout in seconds (default: 30)
    pub timeout_secs: u64,
}
```

### WhatsAppBusinessConfig (Business API)

```rust
pub struct WhatsAppBusinessConfig {
    /// Access token (required, from Meta Business Suite)
    pub access_token: String,

    /// Phone Number ID (required, bot's phone number ID)
    pub phone_number_id: String,

    /// Business Account ID (required)
    pub business_account_id: String,

    /// Webhook verify token (default: cratos_webhook_verify)
    pub webhook_verify_token: String,

    /// Allowed phone numbers (empty = allow all)
    pub allowed_numbers: Vec<String>,

    /// API version (default: v18.0)
    pub api_version: String,
}
```

### Environment Variables

#### Baileys

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `WHATSAPP_BRIDGE_URL` | No | `http://localhost:3001` | Bridge server URL |
| `WHATSAPP_ALLOWED_NUMBERS` | No | empty | Comma-separated allowed numbers |
| `WHATSAPP_TIMEOUT` | No | 30 | Request timeout (seconds) |

#### Business API

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `WHATSAPP_ACCESS_TOKEN` | Yes | - | Meta Access Token |
| `WHATSAPP_PHONE_NUMBER_ID` | Yes | - | Phone Number ID |
| `WHATSAPP_BUSINESS_ACCOUNT_ID` | Yes | - | Business Account ID |
| `WHATSAPP_WEBHOOK_VERIFY_TOKEN` | No | `cratos_webhook_verify` | Webhook verify token |
| `WHATSAPP_ALLOWED_NUMBERS` | No | empty | Comma-separated allowed numbers |
| `WHATSAPP_API_VERSION` | No | `v18.0` | Graph API version |

## Security

### Sensitive Information Masking

Automatic masking to prevent sensitive info in logs:

```rust
// Patterns that trigger [REDACTED]
const SENSITIVE_PATTERNS: &[&str] = &[
    "password", "secret", "token", "api_key",
    "bearer", "credential", "private"
];
```

### Number Filtering

Restrict access to specific phone numbers:

```bash
# Set allowed numbers (include country code)
WHATSAPP_ALLOWED_NUMBERS=+821012345678,+821098765432
```

Numbers are normalized for comparison:
- `+82-10-1234-5678` -> `821012345678`
- `010-1234-5678` -> `1012345678`

### Webhook Verification (Business API)

Meta sends verification requests when setting up webhooks:

```
GET /webhook/whatsapp-business?hub.mode=subscribe&hub.verify_token=YOUR_TOKEN&hub.challenge=CHALLENGE
```

Cratos handles verification automatically:

```rust
pub fn verify_webhook(&self, mode: &str, token: &str, challenge: &str) -> Option<String> {
    if mode == "subscribe" && token == self.config.webhook_verify_token {
        Some(challenge.to_string())
    } else {
        None
    }
}
```

### Access Token Security (Business API)

- Store in environment variables, never hardcode
- Rotate permanent tokens periodically
- Regenerate immediately if token is exposed

## API Reference

### WhatsAppAdapter (Baileys)

```rust
impl WhatsAppAdapter {
    /// Create new adapter
    pub fn new(config: WhatsAppConfig) -> Self;

    /// Create from environment variables
    pub fn from_env() -> Result<Self>;

    /// Check connection status
    pub async fn status(&self) -> Result<ConnectionStatus>;

    /// Start connection (may return QR code)
    pub async fn connect(&self) -> Result<WhatsAppConnection>;

    /// Disconnect
    pub async fn disconnect(&self) -> Result<()>;

    /// Check if connected
    pub fn is_connected(&self) -> bool;

    /// Check if number is allowed
    pub fn is_number_allowed(&self, number: &str) -> bool;

    /// Handle webhook message
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
    /// Create new adapter
    pub fn new(config: WhatsAppBusinessConfig) -> Self;

    /// Create from environment variables
    pub fn from_env() -> Result<Self>;

    /// Verify webhook
    pub fn verify_webhook(&self, mode: &str, token: &str, challenge: &str) -> Option<String>;

    /// Check if number is allowed
    pub fn is_number_allowed(&self, number: &str) -> bool;

    /// Extract messages from webhook
    pub fn extract_messages(&self, webhook: &WhatsAppBusinessWebhook) -> Vec<(String, WebhookMessage)>;

    /// Handle webhook
    pub async fn handle_webhook(
        &self,
        orchestrator: Arc<Orchestrator>,
        webhook: WhatsAppBusinessWebhook,
    ) -> Result<()>;

    /// Mark message as read
    pub async fn mark_as_read(&self, message_id: &str) -> Result<()>;
}
```

### ChannelAdapter Implementation

```rust
impl ChannelAdapter for WhatsAppAdapter {
    /// Send message
    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String>;

    /// Edit message (not supported)
    async fn edit_message(&self, channel_id: &str, message_id: &str, message: OutgoingMessage) -> Result<()>;

    /// Delete message (not supported)
    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()>;

    /// Send typing indicator
    async fn send_typing(&self, channel_id: &str) -> Result<()>;
}
```

## Limitations

### Feature Comparison

| Feature | Baileys | Business API | Notes |
|---------|---------|--------------|-------|
| Send Messages | Yes | Yes | |
| Edit Messages | No | No | WhatsApp doesn't support |
| Delete Messages | No | No | Complex implementation |
| Typing Indicator | Yes | No | API not provided |
| Read Receipts | No | Yes | |
| Group Messages | Yes | No | Requires separate approval |
| Media Messages | No | No | Future support planned |
| Template Messages | No | No | Future support planned |

### Message Length Limits

```rust
// Auto-split messages over 4096 characters
if response_text.len() > 4096 {
    for chunk in response_text.as_bytes().chunks(4096) {
        // Send each chunk
    }
}
```

### Business API Limitations

- **24-hour Window**: Can only freely respond within 24 hours of user's message
- **Template Messages**: Only pre-approved templates after 24-hour window
- **Pricing**: Per-message pricing (varies by country)

## Troubleshooting

### Baileys Issues

#### Bridge Server Connection Failed

```bash
# Check bridge server status
curl http://localhost:3001/status

# Expected response
{"status":"connected","qr":null,"connected":true}
```

#### QR Code Keeps Regenerating

1. Check `auth_info` directory permissions
2. Delete existing session files and retry
3. Disconnect existing web sessions in WhatsApp app

#### Account Ban Warning

WhatsApp may detect suspicious activity:
- Too frequent messages from new device
- Automated message patterns
- Messages to many unknown numbers

**Response**:
1. Stop the bot immediately
2. Wait several hours
3. Reduce message frequency
4. Consider switching to Business API

### Business API Issues

#### "Invalid Access Token" Error

```bash
# Verify token validity
curl "https://graph.facebook.com/v18.0/me?access_token=YOUR_TOKEN"
```

If token expired:
1. Generate new token in Meta Business Suite
2. Consider switching to permanent token

#### Webhook Verification Failed

1. Verify `WHATSAPP_WEBHOOK_VERIFY_TOKEN` matches Meta dashboard setting
2. Ensure endpoint is HTTPS (required)
3. Check firewall/proxy settings

#### API Error Codes

| Code | Meaning | Solution |
|------|---------|----------|
| 190 | Invalid Access Token | Regenerate token |
| 368 | Rate Limit | Reduce request frequency |
| 131030 | 24-hour window exceeded | Use template messages |
| 131047 | Recipient blocked | Try different number |

## Roadmap

1. **v1.1**: Media message support (images, documents)
2. **v1.2**: Template message support (Business API)
3. **v1.3**: Button/list interactive messages
4. **v2.0**: WhatsApp Flows integration
