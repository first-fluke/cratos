# Slack Integration Guide

## Overview

Integrate Cratos as a Slack bot to use the AI assistant in workspace channels or DMs. Features real-time event handling via Socket Mode and robust security capabilities.

### Key Features

| Feature | Description |
|---------|-------------|
| **Channel Chat** | Converse via @mentions in public/private channels |
| **DM Support** | Direct messages (no mention required) |
| **Socket Mode** | Real-time connection behind firewalls |
| **Thread Support** | Maintain conversation context |
| **Access Control** | Workspace/channel-level permissions |
| **Request Signing** | HMAC-SHA256 security verification |
| **Interactive Buttons** | Block Kit-based UI elements |

## Architecture

```
+-------------------------------------------------------------+
|                    Slack Workspace                           |
|  +------------------+  +------------------+                  |
|  |  #general        |  |  @Cratos (DM)    |                  |
|  |  @Cratos hello   |  |  "task request"  |                  |
|  +--------+---------+  +--------+---------+                  |
+-----------|--------------------|-----------------------------+
            |                    |
            v                    v
+-------------------------------------------------------------+
|              Slack API (Socket Mode / Events API)            |
|  +--------------------------------------------------------+ |
|  |  WebSocket Connection (wss://wss-primary.slack.com)    | |
|  +--------------------------------------------------------+ |
+---------------------------+----------------------------------+
                            |
                            v
+-------------------------------------------------------------+
|                     Cratos Server                            |
|  +--------------------------------------------------------+ |
|  |                    SlackAdapter                         | |
|  |  +--------------+  +--------------+  +---------------+  | |
|  |  | slack-       |  | Signature    |  | Message       |  | |
|  |  | morphism     |  | Verifier     |  | Normalizer    |  | |
|  |  | Client       |  | (HMAC-SHA256)|  |               |  | |
|  |  +--------------+  +--------------+  +---------------+  | |
|  +--------------------------------------------------------+ |
|                           |                                  |
|                           v                                  |
|  +--------------------------------------------------------+ |
|  |                    Orchestrator                         | |
|  |         (LLM Processing -> Tool Execution -> Response)  | |
|  +--------------------------------------------------------+ |
+-------------------------------------------------------------+
```

## Setup Guide

### 1. Create Slack App

1. Go to [Slack API Portal](https://api.slack.com/apps)
2. Click "Create New App"
3. Select "From scratch"
4. Enter App name (e.g., "Cratos Assistant")
5. Select workspace and click "Create App"

### 2. Configure OAuth Permissions

In the **OAuth & Permissions** tab, add these Bot Token Scopes:

```
Required Scopes:
[x] chat:write          # Send messages
[x] chat:write.public   # Send messages to public channels
[x] im:history          # Read DM history
[x] im:read             # Read DM channel info
[x] im:write            # Send DMs
[x] channels:history    # Read public channel history
[x] channels:read       # Read public channel info
[x] groups:history      # Read private channel history
[x] groups:read         # Read private channel info
[x] users:read          # Read user info

Optional Scopes:
[x] app_mentions:read   # Subscribe to app mention events
[x] reactions:write     # Add reactions
[x] files:write         # Upload files
```

### 3. Enable Socket Mode

In the **Socket Mode** tab:

1. Toggle "Enable Socket Mode" ON
2. Generate App-Level Token:
   - Token Name: "cratos-socket" (any name)
   - Scope: Add `connections:write`
   - Click "Generate"
3. Copy the `xapp-...` token (store securely)

### 4. Configure Event Subscriptions

In the **Event Subscriptions** tab:

1. Toggle "Enable Events" ON
2. Request URL not needed when using Socket Mode
3. Under **Subscribe to bot events**, add:

```
[x] message.channels     # Public channel messages
[x] message.groups       # Private channel messages
[x] message.im           # DM messages
[x] app_mention          # @mention events
```

### 5. Collect Tokens

You need 3 tokens:

| Token | Location | Format |
|-------|----------|--------|
| Bot Token | OAuth & Permissions | `xoxb-...` |
| App Token | Basic Information > App-Level Tokens | `xapp-...` |
| Signing Secret | Basic Information > App Credentials | 32-char hex |

### 6. Install the App

1. Go to **Install App** tab
2. Click "Install to Workspace"
3. Review permissions and click "Allow"

### 7. Set Environment Variables

```bash
# .env file
# Required settings
SLACK_BOT_TOKEN=xoxb-1234567890-1234567890123-AbCdEfGhIjKlMnOpQrStUvWx
SLACK_APP_TOKEN=xapp-1-A1234567890-1234567890123-abcdefghijklmnopqrstuvwxyz1234567890
SLACK_SIGNING_SECRET=abcdef1234567890abcdef1234567890

# Optional settings
SLACK_ALLOWED_WORKSPACES=T1234567890,T0987654321  # Allowed workspaces (empty = all)
SLACK_ALLOWED_CHANNELS=C1234567890,C0987654321    # Allowed channels (empty = all)
SLACK_MENTIONS_ONLY=true                          # true = respond only to @mentions/DMs
```

## Socket Mode vs Events API

### Socket Mode (Recommended)

```
Advantages:
[+] Works behind firewalls/NAT
[+] No public URL required
[+] Instant connection (no URL verification)
[+] Ideal for development

Disadvantages:
[-] Limited concurrent connections
[-] Not suitable for large-scale deployments
```

### Events API (HTTP Webhook)

```
Advantages:
[+] Unlimited scaling
[+] Load balancing possible
[+] Suitable for large deployments

Disadvantages:
[-] Requires public HTTPS endpoint
[-] Requires Request URL verification
[-] Signature verification mandatory
```

### Connection Mode Selection Guide

| Scenario | Recommended Mode |
|----------|-----------------|
| Development/Testing | Socket Mode |
| Small team (< 100) | Socket Mode |
| Server behind firewall | Socket Mode |
| Large-scale deployment | Events API |
| High availability required | Events API |

## Usage

### Channel Conversations

```
User: @Cratos summarize today's tasks
Cratos: Here's your task summary:
        1. Update API documentation
        2. Improve test coverage
        3. Review performance optimization
```

### DM Conversations

No @mention needed in DMs:

```
User: Please review my code
Cratos: Sure, which PR should I review?

User: #123
Cratos: I've reviewed PR #123...
```

### Thread Context

Conversations in threads maintain context:

```
User: @Cratos write a fibonacci function
Cratos: fn fibonacci(n: u64) -> u64 { ... }

[In thread]
User: Convert it to iterative
Cratos: fn fibonacci_iter(n: u64) -> u64 { ... }
```

## Configuration Options

### SlackConfig

```rust
pub struct SlackConfig {
    /// Bot Token (xoxb-...)
    /// Issued from OAuth & Permissions
    pub bot_token: String,

    /// App Token for Socket Mode (xapp-...)
    /// Issued from Basic Information > App-Level Tokens
    pub app_token: String,

    /// Signing Secret for request verification
    /// Found in Basic Information > App Credentials
    pub signing_secret: String,

    /// Allowed workspace IDs
    /// Empty array = allow all workspaces
    pub allowed_workspaces: Vec<String>,

    /// Allowed channel IDs
    /// Empty array = allow all channels
    pub allowed_channels: Vec<String>,

    /// Mentions-only mode
    /// true: Respond only to @mentions or DMs
    /// false: Respond to all messages
    pub mentions_only: bool,
}
```

### Programmatic Configuration

```rust
use cratos_channels::slack::{SlackAdapter, SlackConfig};

// Builder pattern
let config = SlackConfig::new(
    "xoxb-your-bot-token",
    "xapp-your-app-token",
    "your-signing-secret"
)
.with_allowed_workspaces(vec!["T1234567890".to_string()])
.with_allowed_channels(vec!["C1234567890".to_string(), "C0987654321".to_string()])
.with_mentions_only(true);

let adapter = SlackAdapter::new(config);

// Or create from environment variables
let adapter = SlackAdapter::from_env()?;
```

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SLACK_BOT_TOKEN` | Yes | - | Bot User OAuth Token (xoxb-...) |
| `SLACK_APP_TOKEN` | Yes | - | App-Level Token (xapp-...) |
| `SLACK_SIGNING_SECRET` | Yes | - | Request signature verification secret |
| `SLACK_ALLOWED_WORKSPACES` | No | empty | Comma-separated workspace IDs |
| `SLACK_ALLOWED_CHANNELS` | No | empty | Comma-separated channel IDs |
| `SLACK_MENTIONS_ONLY` | No | true | "true" or "1" enables mention mode |

## Security

### Request Signature Verification (HMAC-SHA256)

Slack includes a signature with all HTTP requests. Cratos verifies this to block forged requests.

```rust
// Verification process
pub fn verify_signature(&self, timestamp: &str, body: &str, signature: &str) -> Result<()> {
    // 1. Validate timestamp (within 5 minutes)
    // 2. Compute HMAC-SHA256 signature
    // 3. Constant-time comparison (prevent timing attacks)
}
```

#### Signature Verification Flow

```
1. Receive Slack request
   Headers:
   - X-Slack-Request-Timestamp: 1531420618
   - X-Slack-Signature: v0=a2114d57b48eac39...

2. Build base string
   sig_basestring = "v0:{timestamp}:{body}"

3. Compute HMAC-SHA256
   expected = HMAC-SHA256(signing_secret, sig_basestring)

4. Compare signatures (constant-time)
   if signature == "v0={expected_hex}" -> OK
   else -> Reject
```

### Replay Attack Prevention

```rust
// Reject requests with timestamps older than 5 minutes
const MAX_TIMESTAMP_AGE_SECS: u64 = 300;

let age = now.abs_diff(request_timestamp);
if age > MAX_TIMESTAMP_AGE_SECS {
    return Err("Request timestamp is too old");
}
```

### Constant-Time Comparison

Uses constant-time comparison to prevent timing attacks:

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

### Access Control

```bash
# Allow specific workspaces only
SLACK_ALLOWED_WORKSPACES=T1234567890

# Allow specific channels only
SLACK_ALLOWED_CHANNELS=C1234567890,C0987654321

# DM channels start with 'D' (auto-detected)
# D1234567890 -> Recognized as DM, no mention required
```

## API Reference

### SlackAdapter

```rust
impl SlackAdapter {
    /// Create new adapter
    pub fn new(config: SlackConfig) -> Self;

    /// Create from environment variables
    pub fn from_env() -> Result<Self>;

    /// Run bot (Socket Mode)
    pub async fn run(self: Arc<Self>, orchestrator: Arc<Orchestrator>) -> Result<()>;

    /// Check if workspace is allowed
    pub fn is_workspace_allowed(&self, workspace_id: &str) -> bool;

    /// Check if channel is allowed
    pub fn is_channel_allowed(&self, channel_id: &str) -> bool;

    /// Check if bot is mentioned
    pub async fn is_bot_mentioned(&self, text: &str) -> bool;

    /// Get bot User ID
    pub async fn get_bot_user_id(&self) -> Option<String>;

    /// Verify request signature
    pub fn verify_signature(
        &self,
        timestamp: &str,
        body: &str,
        signature: &str
    ) -> Result<()>;

    /// Verify webhook request (with headers)
    pub fn verify_webhook_request(
        &self,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<()>;

    /// Process message (called from webhook/socket mode)
    pub async fn process_message(
        &self,
        orchestrator: &Orchestrator,
        channel: &str,
        user: &str,
        text: &str,
        ts: &str,
        thread_ts: Option<&str>,
    ) -> Result<Option<String>>;

    /// Normalize message
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

### ChannelAdapter Implementation

```rust
impl ChannelAdapter for SlackAdapter {
    /// Return channel type
    fn channel_type(&self) -> ChannelType;

    /// Send message
    async fn send_message(
        &self,
        channel_id: &str,
        message: OutgoingMessage
    ) -> Result<String>;

    /// Edit message
    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        message: OutgoingMessage
    ) -> Result<()>;

    /// Delete message
    async fn delete_message(
        &self,
        channel_id: &str,
        message_id: &str
    ) -> Result<()>;

    /// Send typing indicator (not supported for Slack bots)
    async fn send_typing(&self, channel_id: &str) -> Result<()>;
}
```

### OutgoingMessage Usage

```rust
use cratos_channels::message::OutgoingMessage;

// Basic text message
let message = OutgoingMessage::text("Hello, World!");

// Thread reply
let reply = OutgoingMessage::text("Thread reply")
    .in_thread("1234567890.123456".to_string());

// With buttons (Block Kit)
let interactive = OutgoingMessage::text("Choose an option:")
    .with_buttons(vec![
        MessageButton::new("Option A", "option_a"),
        MessageButton::new("Option B", "option_b"),
    ]);
```

## Troubleshooting

### Bot Not Responding

1. **Check Event Subscriptions**
   - Verify `message.channels`, `message.im`, `app_mention` events are subscribed

2. **Check OAuth Scopes**
   - Verify required scopes: `chat:write`, `channels:history`, `im:history`, etc.

3. **Check Channel Invitation**
   - Private channels require explicit bot invitation
   - Run `/invite @Cratos`

4. **Check Mentions Mode**
   ```bash
   # To respond to all messages in public channels
   SLACK_MENTIONS_ONLY=false
   ```

### "invalid_auth" Error

```bash
# Verify token format
# Bot Token: starts with xoxb-
# App Token: starts with xapp-

echo $SLACK_BOT_TOKEN | head -c 5  # xoxb-
echo $SLACK_APP_TOKEN | head -c 5  # xapp-
```

### "missing_scope" Error

```
Error: missing_scope
Needed: chat:write

Solution:
1. Go to api.slack.com/apps and select your app
2. OAuth & Permissions > Scopes
3. Add the required scope
4. Reinstall app (Install App)
```

### Socket Mode Connection Failure

```bash
# Verify App Token has connections:write scope
# Check in Basic Information > App-Level Tokens

# Verify Socket Mode is enabled
# Settings > Socket Mode > Enable Socket Mode: ON
```

### Signature Verification Failed

```
Error: Invalid request signature

Possible causes:
1. SLACK_SIGNING_SECRET value is incorrect
2. Request body was modified (encoding issue)
3. Timestamp is more than 5 minutes old (replay attack detected)

Verify:
- Basic Information > App Credentials > Signing Secret
- Ensure exact copy without whitespace
```

### Rate Limiting

```
Error: ratelimited

Slack API limits:
- Tier 1: 1+ per minute
- Tier 2: 20+ per minute
- Tier 3: 50+ per minute
- Tier 4: 100+ per minute

Solution:
- chat.postMessage: Tier 3 (recommend 1 msg/sec per channel)
- Check Retry-After header when retrying
```

### Finding Channel IDs

```
Method 1: Right-click channel > "Copy link"
https://workspace.slack.com/archives/C1234567890
                                      ^^^^^^^^^^^
                                      Channel ID

Method 2: Channel details > ID shown at bottom

Channel ID formats:
- C... : Public channel
- G... : Private channel
- D... : DM
- T... : Workspace
```

## Roadmap

1. **v1.1**: Slash command support (`/cratos ask ...`)
2. **v1.2**: Modal/Home tab support
3. **v1.3**: File upload/download
4. **v2.0**: Workflow Builder integration
