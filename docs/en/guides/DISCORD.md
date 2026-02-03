# Discord Integration Guide

## Overview

Integrate Cratos as a Discord bot to use the AI assistant in servers (guilds) or DMs.

### Key Features

| Feature | Description |
|---------|-------------|
| **Server Chat** | Converse via @mention in guild channels |
| **DM Support** | 1:1 direct messages |
| **Permission Management** | Configure allowed guilds/channels |
| **Threads** | Maintain reply context |
| **Typing Indicator** | Show typing while responding |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Discord Server                           │
│  ┌─────────────────┐  ┌─────────────────┐                  │
│  │  #general       │  │  #dev-chat      │                  │
│  │  @Cratos hello  │  │                 │                  │
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
│  │         (LLM Processing → Tool Execution → Response)     ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## Setup

### 1. Create Discord Bot

1. Go to [Discord Developer Portal](https://discord.com/developers/applications)
2. Click "New Application"
3. Enter app name (e.g., "Cratos Assistant")
4. Go to "Bot" tab → Click "Add Bot"
5. "Reset Token" → Copy token (⚠️ shown only once)

### 2. Configure Bot Permissions

Enable these in the "Bot" tab:

```
✅ MESSAGE CONTENT INTENT (required!)
✅ Send Messages
✅ Read Message History
✅ Add Reactions
✅ Use Slash Commands (optional)
```

### 3. Invite Bot

In "OAuth2" → "URL Generator":

```
Scopes:
✅ bot
✅ applications.commands (optional)

Bot Permissions:
✅ Send Messages
✅ Read Message History
✅ Add Reactions
```

Use the generated URL to invite the bot to your server

### 4. Set Environment Variables

```bash
# .env
DISCORD_BOT_TOKEN=your_bot_token_here

# Optional
DISCORD_ALLOWED_GUILDS=123456789,987654321  # Allowed server IDs (empty = all servers)
DISCORD_ALLOWED_CHANNELS=111222333          # Allowed channel IDs (empty = all channels)
DISCORD_REQUIRE_MENTION=true                # Require @mention in servers
```

## Usage

### In Server Channels

```
User: @Cratos What's the weather today?
Cratos: The current weather in Seoul is...

User: @Cratos Create a fibonacci function
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

### In DMs

No @mention needed in DMs:

```
User: Hello!
Cratos: Hello! How can I help you?
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

## Configuration Options

### DiscordConfig

```rust
pub struct DiscordConfig {
    /// Bot token (required)
    pub bot_token: String,

    /// Allowed guild IDs (empty = allow all)
    pub allowed_guilds: Vec<u64>,

    /// Allowed channel IDs (empty = allow all)
    pub allowed_channels: Vec<u64>,

    /// Require @mention in guild channels (default: true)
    pub require_mention: bool,
}
```

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DISCORD_BOT_TOKEN` | ✅ | - | Bot token |
| `DISCORD_ALLOWED_GUILDS` | ❌ | empty | Comma-separated server IDs |
| `DISCORD_ALLOWED_CHANNELS` | ❌ | empty | Comma-separated channel IDs |
| `DISCORD_REQUIRE_MENTION` | ❌ | true | If false, respond to all messages |

## Security

### Sensitive Information Masking

Automatic masking to prevent sensitive info in logs:

```rust
// Patterns that trigger [REDACTED]
const SENSITIVE_PATTERNS: &[&str] = &[
    "password", "secret", "token", "api_key",
    "bearer", "authorization", "credential", "ssh"
];
```

### Error Message Sanitization

Prevent internal error exposure to users:

```rust
// Internal: "Invalid token: sk-abc123..."
// User sees: "An authentication error occurred."
```

### Permission Restrictions

```bash
# Allow specific servers only
DISCORD_ALLOWED_GUILDS=123456789

# Allow specific channels only
DISCORD_ALLOWED_CHANNELS=111222333,444555666
```

## Message Limits

Automatic handling of Discord's 2000 character limit:

```rust
// Long responses are automatically split into multiple messages
let chunks: Vec<&str> = response_text
    .as_bytes()
    .chunks(2000)
    .filter_map(|chunk| std::str::from_utf8(chunk).ok())
    .collect();
```

## API Reference

### DiscordAdapter

```rust
impl DiscordAdapter {
    /// Create new adapter
    pub fn new(config: DiscordConfig) -> Self;

    /// Create from environment variables
    pub fn from_env() -> Result<Self>;

    /// Run the bot
    pub async fn run(self: Arc<Self>, orchestrator: Arc<Orchestrator>) -> Result<()>;

    /// Check if guild is allowed
    pub fn is_guild_allowed(&self, guild_id: u64) -> bool;

    /// Check if channel is allowed
    pub fn is_channel_allowed(&self, channel_id: u64) -> bool;
}
```

### ChannelAdapter Implementation

```rust
impl ChannelAdapter for DiscordAdapter {
    /// Send message
    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String>;

    /// Edit message
    async fn edit_message(&self, channel_id: &str, message_id: &str, message: OutgoingMessage) -> Result<()>;

    /// Delete message
    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()>;

    /// Send typing indicator
    async fn send_typing(&self, channel_id: &str) -> Result<()>;
}
```

## Troubleshooting

### Bot Not Responding

1. Verify **MESSAGE CONTENT INTENT** is enabled
2. Check bot has read/write permissions in channel
3. If `DISCORD_REQUIRE_MENTION=true`, ensure you @mentioned the bot
4. Verify server ID is in `DISCORD_ALLOWED_GUILDS`

### "Invalid Token" Error

```bash
# Check token format (separated by 2 dots)
# Correct format: OTk...NzY.Gh...Qw.zI...9A
echo $DISCORD_BOT_TOKEN
```

### Rate Limit

Automatic retry on Discord API rate limits. For excessive requests:

```
⚠️ 429 Too Many Requests
→ Automatic retry after delay
```

## Roadmap

1. **v1.1**: Slash command support (`/cratos ask ...`)
2. **v1.2**: Voice channel integration
3. **v2.0**: Embed messages, button interactions
