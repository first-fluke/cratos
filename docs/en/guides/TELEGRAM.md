# Telegram Integration Guide

## Overview

Integrate Cratos as a Telegram bot to use the AI assistant in private chats or groups.

### Key Features

| Feature | Description |
|---------|-------------|
| **Private Chat** | 1:1 direct messages |
| **Group Support** | Converse via @mention in groups/supergroups |
| **Permission Management** | Configure allowed users/groups |
| **Reply Context** | Maintain conversation flow with reply chains |
| **Typing Indicator** | Show typing while responding |
| **Attachments** | Support for images and documents |
| **Inline Keyboard** | Button-based interactions |
| **Markdown** | MarkdownV2 formatted responses |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Telegram                                  │
│  ┌─────────────────┐  ┌─────────────────┐                  │
│  │  Private Chat   │  │  Group Chat      │                  │
│  │  "Summarize"    │  │  @Cratos hello   │                  │
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
│  │  │ Bot       │  │ Handler   │  │ (Masking/Sanitize)│   ││
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

### 1. Create Bot with BotFather

1. Search for [@BotFather](https://t.me/BotFather) in Telegram
2. Send `/newbot` command
3. Enter bot name (e.g., "Cratos Assistant")
4. Enter bot username (e.g., `cratos_assistant_bot`, must end with `_bot`)
5. Copy the token (format: `123456789:ABCdefGHIjklMNOpqrsTUVwxyz`)

```
BotFather: Done! Congratulations on your new bot.

Use this token to access the HTTP API:
123456789:ABCdefGHIjklMNOpqrsTUVwxyz

Keep your token secure and store it safely.
```

### 2. Configure Bot (Optional)

Additional settings via BotFather:

```
/setdescription - Set bot description
/setabouttext - Set bot about text
/setuserpic - Set bot profile picture
/setcommands - Set command list
```

Example command list:
```
help - Show help
status - Check status
cancel - Cancel current task
```

### 3. Set Environment Variables

```bash
# .env
TELEGRAM_BOT_TOKEN=123456789:ABCdefGHIjklMNOpqrsTUVwxyz

# Optional
TELEGRAM_ALLOWED_USERS=123456789,987654321  # Allowed user IDs (empty = allow all)
TELEGRAM_ALLOWED_GROUPS=-100123456789       # Allowed group IDs (empty = allow all)
TELEGRAM_GROUPS_MENTION_ONLY=true           # Respond only to @mentions/replies in groups
```

### 4. How to Find User/Group IDs

**Finding User ID:**
1. Send a message to [@userinfobot](https://t.me/userinfobot)
2. Or use [@getmyid_bot](https://t.me/getmyid_bot)

**Finding Group ID:**
1. Add your bot to the group
2. Send any message in the group
3. Check in browser:
   ```
   https://api.telegram.org/bot<TOKEN>/getUpdates
   ```
4. Find the `chat.id` value (negative for groups)

## Usage

### In Private Chats

Converse directly without @mention:

```
User: Hello!
Cratos: Hello! How can I help you?

User: Create a fibonacci function
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

### In Groups

When `TELEGRAM_GROUPS_MENTION_ONLY=true` (default):

```
[Regular message - ignored]
UserA: What should we have for lunch?

[Invoke with @mention]
UserB: @cratos_bot What's the weather today?
Cratos: The current weather is...

[Invoke with reply]
UserA: (replying to Cratos message) What about tomorrow?
Cratos: Tomorrow's forecast is...
```

### Reply Context

Reply to previous messages to maintain context:

```
User: How do I sort a list in Python?
Cratos: You can use sorted() function or .sort() method...

[Reply to above message]
User: What about descending order?
Cratos: Use the reverse=True parameter...
```

### Attachments

Attach images or documents:

```
User: [Image attached] Review this code
Cratos: I've analyzed the image. In this code, I notice...

User: [PDF attached] Summarize this document
Cratos: Here's a summary of the document...
```

## Configuration Options

### TelegramConfig

```rust
pub struct TelegramConfig {
    /// Bot token (required)
    pub bot_token: String,

    /// Allowed user IDs (empty = allow all)
    pub allowed_users: Vec<i64>,

    /// Allowed group IDs (empty = allow all)
    pub allowed_groups: Vec<i64>,

    /// Respond only to @mentions/replies in groups (default: true)
    pub groups_mention_only: bool,
}
```

### Programmatic Configuration

```rust
use cratos_channels::telegram::{TelegramAdapter, TelegramConfig};

// Basic configuration
let config = TelegramConfig::new("YOUR_BOT_TOKEN");

// Detailed configuration
let config = TelegramConfig::new("YOUR_BOT_TOKEN")
    .with_allowed_users(vec![123456789, 987654321])
    .with_allowed_groups(vec![-100123456789])
    .with_groups_mention_only(true);

let adapter = TelegramAdapter::new(config);

// Or create from environment variables
let adapter = TelegramAdapter::from_env()?;
```

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `TELEGRAM_BOT_TOKEN` | Yes | - | Bot token |
| `TELEGRAM_ALLOWED_USERS` | No | empty | Comma-separated user IDs |
| `TELEGRAM_ALLOWED_GROUPS` | No | empty | Comma-separated group IDs |
| `TELEGRAM_GROUPS_MENTION_ONLY` | No | true | If false, respond to all messages in groups |

## Security

### Sensitive Information Masking

Automatic masking to prevent sensitive info in logs:

```rust
// Patterns that trigger [REDACTED]
const SENSITIVE_PATTERNS: &[&str] = &[
    "password", "passwd", "secret", "token",
    "api_key", "apikey", "api-key", "bearer",
    "authorization", "credential", "private",
    "ssh", "-----begin"
];

// Example
// Input: "my password is secret123"
// Log: "[REDACTED - potentially sensitive content]"
```

### Long Message Truncation

Messages over 50 characters are automatically truncated in logs:

```rust
const MAX_LOG_TEXT_LENGTH: usize = 50;

// Input: "This is a very long message that..."
// Log: "This is a very long message that co...[truncated]"
```

### Error Message Sanitization

Prevent internal error exposure to users:

```rust
// Internal: "Invalid token: sk-abc123..."
// User sees: "An authentication error occurred. Please check your configuration."

// Internal: "Connection timeout to database"
// User sees: "A network error occurred. Please try again later."

// Internal: "SQL error: SELECT * FROM users"
// User sees: "A database error occurred. Please try again later."
```

### Permission Restrictions

```bash
# Allow specific users only
TELEGRAM_ALLOWED_USERS=123456789

# Allow specific groups only
TELEGRAM_ALLOWED_GROUPS=-100123456789,-100987654321

# Combined usage
TELEGRAM_ALLOWED_USERS=123456789
TELEGRAM_ALLOWED_GROUPS=-100123456789
```

## API Reference

### TelegramAdapter

```rust
impl TelegramAdapter {
    /// Create new adapter
    pub fn new(config: TelegramConfig) -> Self;

    /// Create from environment variables
    pub fn from_env() -> Result<Self>;

    /// Get underlying teloxide Bot instance
    pub fn bot(&self) -> &Bot;

    /// Check if user is allowed
    pub fn is_user_allowed(&self, user_id: i64) -> bool;

    /// Check if group is allowed
    pub fn is_group_allowed(&self, chat_id: i64) -> bool;

    /// Convert Telegram message to normalized message
    pub fn normalize_message(
        &self,
        msg: &TelegramMessage,
        bot_username: &str
    ) -> Option<NormalizedMessage>;

    /// Run the bot
    pub async fn run(
        self: Arc<Self>,
        orchestrator: Arc<Orchestrator>
    ) -> Result<()>;
}
```

### TelegramConfig

```rust
impl TelegramConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self>;

    /// Create with token
    pub fn new(bot_token: impl Into<String>) -> Self;

    /// Set allowed users (builder pattern)
    pub fn with_allowed_users(self, users: Vec<i64>) -> Self;

    /// Set allowed groups (builder pattern)
    pub fn with_allowed_groups(self, groups: Vec<i64>) -> Self;

    /// Set groups mention-only mode (builder pattern)
    pub fn with_groups_mention_only(self, enabled: bool) -> Self;
}
```

### ChannelAdapter Implementation

```rust
impl ChannelAdapter for TelegramAdapter {
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

    /// Send typing indicator
    async fn send_typing(&self, channel_id: &str) -> Result<()>;
}
```

### OutgoingMessage Options

```rust
pub struct OutgoingMessage {
    /// Message text
    pub text: String,
    /// Whether to parse markdown
    pub parse_markdown: bool,
    /// Reply-to message ID
    pub reply_to: Option<String>,
    /// Inline keyboard buttons
    pub buttons: Vec<MessageButton>,
}
```

### Using Inline Keyboards

```rust
use cratos_channels::message::{MessageButton, OutgoingMessage};

let message = OutgoingMessage {
    text: "Please choose:".to_string(),
    parse_markdown: false,
    reply_to: None,
    buttons: vec![
        MessageButton::callback("Yes", "approve:yes"),
        MessageButton::callback("No", "approve:no"),
        MessageButton::url("View Docs", "https://docs.example.com"),
    ],
};

adapter.send_message("123456789", message).await?;
```

## Troubleshooting

### Bot Not Responding

1. **Verify token**
   ```bash
   # Check token format (numbers:alphanumeric)
   echo $TELEGRAM_BOT_TOKEN
   # Correct format: 123456789:ABCdefGHIjklMNOpqrsTUVwxyz
   ```

2. **Check permission settings**
   ```bash
   # Verify you're in allowed users/groups
   echo $TELEGRAM_ALLOWED_USERS
   echo $TELEGRAM_ALLOWED_GROUPS
   ```

3. **Check mention mode**
   ```bash
   # If true, @mention or reply required
   echo $TELEGRAM_GROUPS_MENTION_ONLY
   ```

4. **Bot privacy mode**
   - In BotFather: `/setprivacy` then `Disable`
   - Enables receiving all group messages

### Not Receiving Messages in Groups

1. Verify bot is a group administrator
2. Or disable privacy mode via BotFather: `/setprivacy` then `Disable`

### "Unauthorized" Error

```bash
# Token expired or invalid
# Issue new token via BotFather: /token
```

### Markdown Parsing Failures

MarkdownV2 requires escaping special characters:

```rust
// Characters that need escaping: _ * [ ] ( ) ~ ` > # + - = | { } . !
let escaped = text
    .replace("_", "\\_")
    .replace("*", "\\*")
    .replace("[", "\\[")
    // ...
```

Fallback to plain text:
```rust
// Automatically resend as plain text if markdown fails
if send_result.is_err() {
    bot.send_message(chat_id, &response_text).await;
}
```

### Rate Limit (429 Error)

Telegram Bot API limits:
- 30 messages per second (global)
- 20 messages per minute to same group

```
Warning: Too Many Requests: retry after 30
→ Automatic retry after 30 seconds
```

## Attachment Handling

### Supported Types

| Type | AttachmentType | Description |
|------|----------------|-------------|
| Photo | `Image` | JPEG format, highest resolution selected |
| Document | `Document` | Any file type |

### Attachment Information

```rust
pub struct Attachment {
    /// Attachment type
    pub attachment_type: AttachmentType,
    /// File name (documents only)
    pub file_name: Option<String>,
    /// MIME type
    pub mime_type: Option<String>,
    /// File size (bytes)
    pub file_size: Option<u64>,
    /// Download URL
    pub url: Option<String>,
    /// Telegram file ID
    pub file_id: Option<String>,
}
```

## Roadmap

1. **v1.1**: Callback query handling (button click processing)
2. **v1.2**: File upload/download support
3. **v1.3**: Inline mode support (`@bot query`)
4. **v2.0**: Webhook mode support (instead of polling)
