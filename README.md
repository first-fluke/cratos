# Cratos - AI-Powered Personal Assistant

Cratos is a **Rust-based AI assistant** that understands natural language commands from Telegram/Slack, gathers information, executes tasks, and reports results.

## One-Line Installation

### macOS / Linux
```bash
curl -sSL https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.sh | sh
```

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.ps1 | iex
```

The installer automatically:
- Downloads the appropriate binary for your platform
- Installs to your PATH
- Launches the setup wizard

## Key Features

- **Lightweight**: Runs immediately with embedded SQLite (`~/.cratos/cratos.db`)
- **Automatic Skill Generation**: Learns usage patterns to automatically create workflow skills
- **Multi-LLM Support**: OpenAI, Anthropic, Gemini, DeepSeek, Groq, Fireworks, SiliconFlow, GLM, Qwen, Moonshot, Novita, OpenRouter, Ollama (13 providers, 6 free)
- **Smart Routing**: Automatic model selection by task type reduces costs by 70%
- **Free Model Support**: Free LLMs: Z.AI GLM-4.7-Flash (unlimited), Gemini Flash, Groq, Novita, SiliconFlow
- **Replay Engine**: All executions stored as events, timeline view and replay
- **Tool System**: 23 built-in tools (file ops, HTTP, Git/GitHub, shell exec, PTY bash, browser, web search, agent CLI, WoL, config, image generation, file transfer, native app automation) + MCP extensibility
- **Channel Adapters**: Telegram, Slack, Discord, Matrix, WhatsApp — with slash commands, DM policy, EventBus notifications
- **Chrome Extension**: Browser control via Chrome extension + WebSocket gateway protocol
- **Graph RAG Memory**: Cross-session conversation memory with entity graph + hybrid vector search
- **TUI Chat**: ratatui-based interactive terminal with markdown rendering, mouse scroll, input history, multi-provider quota display
- **Voice Control**: STT (Whisper API / local Whisper) + TTS (Edge TTS) + VAD (Silero), supports ko/en/ja/zh
- **Web Search**: Built-in DuckDuckGo search (no API key required)
- **MCP Integration**: Auto-discovery of MCP servers from `.mcp.json`, SSE/stdio support
- **Proactive Scheduler**: Cron, interval, one-time, file-watch, and system-event triggers
- **Security**: Auth middleware (HMAC/JWT/API Key), rate limiting, Docker sandbox, credential encryption, prompt injection defense
- **Olympus OS**: Mythology-based 3-layer agent organization (Pantheon/Decrees/Chronicles)
- **ACP Bridge**: IDE integration via stdin/stdout JSON-lines protocol
- **Device Pairing**: PIN-based mobile pairing for remote device management
- **Remote Development**: Issue → PR end-to-end automation (`cratos develop`)

## System Requirements

| Item | Minimum¹ | Recommended | Optimal |
|------|----------|-------------|---------|
| **OS** | macOS 11+, Windows 10, Ubuntu 20.04+ | macOS 12+, Windows 10+, Ubuntu 22.04+ | Latest |
| **CPU** | 1 core | 1 core | 2+ cores |
| **RAM** | 256MB (runtime) / 2GB (build) | 1GB (runtime) / 4GB (build) | 4GB+ |
| **Disk** | 100MB | 1GB | 5GB+ |
| **Rust** | 1.88+ | 1.88+ | Latest stable |
| **Docker** | - | Optional | Latest |

> ¹ **Minimum**: With embeddings disabled (`cargo build --no-default-features`). Semantic search unavailable.
>
> **Note**: No PostgreSQL or Docker required. Data is stored in `~/.cratos/cratos.db` (SQLite).

### Ollama Local LLM (Optional)

| Model | RAM | VRAM (GPU) |
|-------|-----|------------|
| Llama 3.2 3B | 4GB | 4GB |
| Llama 3.2 7B | 8GB | 8GB |
| Llama 3.1 70B | 48GB | 48GB |

> **Note**: No GPU required when using external LLM APIs (OpenAI, Anthropic, etc.)

## Quick Start

### Option 1: One-Line Install (Recommended)

```bash
# macOS / Linux
curl -sSL https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.ps1 | iex
```

The setup wizard will guide you through:
1. Creating a Telegram bot
2. Choosing an AI provider (free options available)
3. Testing your configuration

### Option 2: Manual Setup

```bash
# Clone the repository
git clone https://github.com/first-fluke/cratos.git
cd cratos

# Run the setup wizard
cargo run -- init

# With Korean language
cargo run -- init --lang ko
```

### Option 3: Build from Source

```bash
# Create environment file
cp .env.example .env

# Edit .env file (add your API keys)
vim .env

# Build and run
cargo build --release
cargo run --release

# Health check
curl http://localhost:19527/health
```

Data is automatically stored in `~/.cratos/cratos.db`.

### Setup

| Command | Description |
|---------|-------------|
| `cratos init` | Unified interactive setup wizard (auto-detects language) |
| `cratos init --lang ko` | Setup wizard in Korean |

## Project Structure

```
cratos/
├── crates/
│   ├── cratos-core/      # Orchestration engine, security, credentials, shutdown
│   ├── cratos-channels/  # Channel adapters (Telegram, Slack, Discord, Matrix, WhatsApp)
│   ├── cratos-tools/     # Tool registry, sandbox, MCP client, browser relay
│   ├── cratos-llm/       # LLM providers, token counting, ONNX embeddings, quota tracking
│   ├── cratos-replay/    # Event logging and replay (SQLite)
│   ├── cratos-skills/    # Automatic skill generation system
│   ├── cratos-search/    # Vector search (usearch), semantic indexing
│   ├── cratos-memory/    # Graph RAG conversation memory (entity graph + hybrid search)
│   ├── cratos-crypto/    # Cryptographic utilities
│   ├── cratos-audio/     # Voice control (STT/TTS, optional)
│   └── cratos-canvas/    # Live Canvas (future)
├── config/
│   ├── default.toml      # Default configuration
│   ├── pantheon/         # Persona TOML files (14 personas: 5 core + 9 extended)
│   └── decrees/          # Laws, ranks, development rules
├── src/
│   ├── main.rs           # Application entry point
│   ├── cli/              # CLI commands (init, doctor, quota, tui, skill, data, acp, browser-ext, ...)
│   ├── api/              # REST API (config, tools, executions, scheduler, quota, sessions, browser)
│   ├── websocket/        # WebSocket handlers (chat, events, gateway)
│   └── server.rs         # Server initialization

~/.cratos/                # Data directory (auto-created)
├── cratos.db             # SQLite main DB (events, execution history)
├── skills.db             # SQLite skills DB (skills, patterns)
├── memory.db             # SQLite Graph RAG memory DB
├── vectors/              # HNSW vector index (usearch)
│   └── memory/           # Memory embedding vectors
└── chronicles/           # Achievement records per persona
```

## Configuration

### Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `REDIS_URL` | Redis connection URL (for sessions, uses memory if not set) | |
| `TELEGRAM_BOT_TOKEN` | Telegram bot token | △ |
| `SLACK_BOT_TOKEN` | Slack bot token | △ |
| **LLM API Keys (at least one)** | | |
| `OPENAI_API_KEY` | OpenAI API key | |
| `ANTHROPIC_API_KEY` | Anthropic API key | |
| `GEMINI_API_KEY` | Google Gemini API key (or GOOGLE_API_KEY) (recommended) | |
| `GOOGLE_API_KEY` | Google Gemini API key (alias) | |
| `ZHIPU_API_KEY` | Z.AI GLM API key (free Flash models) | |
| `DASHSCOPE_API_KEY` | Alibaba Qwen API key | |
| `OPENROUTER_API_KEY` | OpenRouter API key | |
| `NOVITA_API_KEY` | Novita AI API key (free) | |
| `ELEVENLABS_API_KEY` | ElevenLabs TTS API key (optional) | |
| **Configuration Overrides** | | |
| `CRATOS_LLM__DEFAULT_PROVIDER` | Override default LLM provider (double underscore) | |

> **Note**: `DATABASE_URL` is no longer needed. Uses embedded SQLite.

### Configuration Files

Default settings are in `config/default.toml`. Create `config/local.toml` to customize for your local environment.

## LLM Providers

### Paid Providers

| Provider | Models | Features |
|----------|--------|----------|
| **OpenAI** | GPT-5, GPT-5.2, GPT-5-nano | Latest generation, coding |
| **Anthropic** | Claude Sonnet 4.5, Claude Haiku 4.5, Claude Opus 4.5 | Excellent code generation |
| **Gemini** | Gemini 3 Pro, Gemini 3 Flash, Gemini 2.5 Pro | Long context, multimodal, Standard API only (safe) |
| **GLM** | GLM-4.7, GLM-4.7-Flash (free), GLM-5 | ZhipuAI models |
| **Qwen** | Qwen3-Max, Qwen3-Plus, Qwen3-Flash, Qwen3-Coder | Multilingual, coding, reasoning |
| **DeepSeek** | DeepSeek-V3.2, DeepSeek-R1 | Ultra low cost, reasoning |

### Free/Low-Cost Providers

| Provider | Models | Limits |
|----------|--------|--------|
| **Z.AI (GLM)** | GLM-4.7-Flash, GLM-4.5-Flash | Free, no daily limit |
| **Gemini** | Gemini 2.0 Flash | Free (1,500 RPD) |
| **Groq** | Llama 3.1 8B, GPT-OSS 20B | Free tier available |
| **Novita** | Qwen2.5-7B, GLM-4-9B | Free signup |
| **SiliconFlow** | Qwen2.5-7B | Free models available |
| **Ollama** | All local models | Unlimited (local) |

### Model Routing

Automatic model selection based on task type:

| Task Type | Model Tier | Example Models |
|-----------|------------|----------------|
| Classification | Fast | GPT-5-nano, Claude Haiku 4.5 |
| Summarization | Fast | GPT-5-nano, Gemini 2.0 Flash |
| Conversation | Standard | GPT-5, Claude Sonnet 4.5 |
| CodeGeneration | Standard | GPT-5, Claude Sonnet 4.5 |
| Planning | Premium | GPT-5.2, Claude Opus 4.5 |

## Olympus OS (Agent Organization)

Cratos features a mythology-based 3-layer agent organization system:

| Layer | Name | Purpose |
|-------|------|---------|
| WHO | **Pantheon** | 14 agent personas (5 core + 9 extended) |
| HOW | **Decrees** | Laws, ranks, development rules |
| WHAT | **Chronicles** | Achievement records and evaluations |

### Core Personas

| Role | Name | Domain |
|------|------|--------|
| Orchestrator | **Cratos** | Supreme commander (Lv255) |
| PM | **Athena** | Strategy, planning (Lv3) |
| DEV | **Sindri** | Development, implementation (Lv1) |
| QA | **Heimdall** | Quality, security (Lv2) |
| RESEARCHER | **Mimir** | Research, analysis (Lv4) |

### Extended Personas

| Role | Name | Domain |
|------|------|--------|
| PO | **Odin** | Product owner (Lv5) |
| HR | **Hestia** | People, organization (Lv2) |
| BA | **Norns** | Business analysis (Lv3) |
| UX | **Apollo** | UX design (Lv3) |
| CS | **Freya** | Customer support (Lv2) |
| LEGAL | **Tyr** | Legal, compliance (Lv4) |
| MARKETING | **Nike** | Marketing (Lv2) |
| DEVOPS | **Thor** | Infrastructure, ops (Lv3) |
| DEV | **Brok** | Development (Lv1) |

### @mention Routing

Route tasks to specific personas using @mentions:

```
@athena Plan this sprint          # PM - Strategy
@sindri Implement the API         # DEV - Development
@heimdall Review security         # QA - Quality
@mimir Research this technology   # RESEARCHER - Analysis
@cratos Summarize the situation   # Orchestrator
```

Response format: `[Persona LvN] Per Laws Article N...`

### CLI Commands

```bash
# Setup
cratos init                       # Interactive setup wizard (auto-detects language)
cratos init --lang ko             # Setup wizard in Korean

# System
cratos serve                      # Start the server
cratos doctor                     # Run diagnostics
cratos quota                      # Show provider quota/cost status
cratos quota --watch              # Live-refresh mode (every 2s)
cratos quota --json               # JSON output for scripting
cratos tui                        # Launch interactive TUI chat
cratos tui --persona sindri       # TUI with specific persona
cratos acp                        # Start ACP bridge (IDE integration)

# Voice
cratos voice                      # Start voice assistant (default: ko)
cratos voice --lang en            # Voice assistant in English

# Remote Development
cratos develop --repo user/repo   # Issue → PR automation
cratos develop --dry-run          # Preview without changes

# Device Pairing
cratos pair start                 # Start PIN-based pairing
cratos pair devices               # List paired devices
cratos pair unpair <device>       # Unpair a device

# Browser
cratos browser tabs               # List open browser tabs
cratos browser open <url>         # Open a URL
cratos browser screenshot         # Capture screenshot
cratos browser extension install  # Install Chrome extension

# Security
cratos security audit             # Run security audit

# Skills
cratos skill list                 # List all skills
cratos skill show <name>          # Show skill details
cratos skill enable <name>        # Enable a skill
cratos skill disable <name>       # Disable a skill
cratos skill export <name>        # Export a skill to file
cratos skill import <file>        # Import a skill from file
cratos skill bundle               # Bundle skills for sharing
cratos skill search <query>       # Search remote skill registry
cratos skill install <name>       # Install from registry
cratos skill publish <name>       # Publish to registry

# Data Management
cratos data stats                 # Show database statistics
cratos data clear sessions        # Clear session data
cratos data clear memory          # Clear Graph RAG memory
cratos data clear history         # Clear execution history
cratos data clear chronicles      # Clear achievement records
cratos data clear vectors         # Clear vector indices
cratos data clear skills          # Clear learned skills

# Pantheon (Personas)
cratos pantheon list              # List personas
cratos pantheon show sindri       # Show persona details
cratos pantheon summon sindri     # Summon (activate) a persona
cratos pantheon dismiss           # Dismiss active persona

# Pantheon Skill Management
cratos pantheon skill list <persona>       # List skills bound to persona
cratos pantheon skill show <persona> <skill>   # Show binding details
cratos pantheon skill claim <persona> <skill>  # Manually assign skill
cratos pantheon skill release <persona> <skill> # Release skill
cratos pantheon skill leaderboard <skill>  # Skill leaderboard
cratos pantheon skill summary <persona>    # Persona skill summary
cratos pantheon skill sync <persona>       # Sync proficiency to chronicle

# Decrees (Rules)
cratos decrees show laws          # Show laws
cratos decrees show ranks         # Show rank system
cratos decrees show warfare       # Show development rules
cratos decrees show alliance      # Show collaboration rules
cratos decrees show tribute       # Show reward/cost rules
cratos decrees show judgment      # Show evaluation framework
cratos decrees show culture       # Show culture/values
cratos decrees show operations    # Show operational procedures
cratos decrees validate           # Validate rule compliance

# Chronicles (Achievement Records)
cratos chronicle list             # List achievement records
cratos chronicle show sindri      # Show individual record
cratos chronicle log "message"    # Add log entry
cratos chronicle promote sindri   # Request promotion
cratos chronicle clean            # Clean up stale records
```

## Security Features

> **Security-first by design** — Built from the ground up with security as a core principle, not an afterthought.

- **Memory-safe foundation**: Written in Rust with `#![forbid(unsafe_code)]` — no buffer overflows, no use-after-free
- **Zero plaintext secrets**: All credentials encrypted via OS keychain (Keychain, Secret Service, Credential Manager)
- **Default isolation**: Sandbox enabled by default with network blocked, not opt-in
- **Built-in threat detection**: 20+ prompt injection patterns detected and blocked automatically
- **Tool risk classification**: Every tool has explicit risk levels with appropriate safeguards
- **Input/output validation**: All user inputs and LLM outputs are validated before execution

### Docker Sandbox

Dangerous tools run in isolated Docker containers:

```toml
[security.sandbox]
default_network = "none"  # Block network
max_memory_mb = 512       # Memory limit
max_cpu_percent = 50      # CPU limit
```

### Credential Encryption

API keys are securely stored in OS keychain:
- macOS: Keychain
- Linux: Secret Service (GNOME Keyring)
- Windows: Credential Manager

### Prompt Injection Defense

Automatically detects and blocks malicious prompts:
- 20+ danger pattern detection
- Input/output validation
- Prevents sensitive information exposure

## Supported Tools

### Built-in Tools

| Tool | Description | Risk Level |
|------|-------------|------------|
| `file_read` | Read files | Low |
| `file_write` | Write files | Medium |
| `file_list` | List directory | Low |
| `http_get` | HTTP GET request | Low |
| `http_post` | HTTP POST request | Medium |
| `exec` | Command execution (meta-char blocked, sandboxed) | High |
| `bash` | PTY-based shell (5-layer security: validation, pipeline analysis, env isolation, resource limits, output masking) | High |
| `git_status` | Git status check | Low |
| `git_commit` | Git commit creation | Medium |
| `git_branch` | Git branch management | Medium |
| `git_diff` | Git diff check | Low |
| `git_push` | Git push to remote | High |
| `git_clone` | Clone a repository | Medium |
| `git_log` | View commit history | Low |
| `github_api` | GitHub API integration | Medium |
| `browser` | Browser automation (MCP or Chrome extension) | Medium |
| `web_search` | DuckDuckGo web search (no API key required) | Low |
| `agent_cli` | Delegate tasks to external AI agents (Claude, etc.) | Medium |
| `wol` | Wake-on-LAN | Medium |
| `config` | Natural language configuration | Medium |
| `send_file` | Send file through messaging channel | Medium |
| `image_generate` | AI image generation | Medium |
| `app_control` | Native app automation (macOS AppleScript/JXA) | High |

### MCP Extension Tools

Additional tools can be auto-registered from `.mcp.json`:

```json
{
  "mcpServers": {
    "playwright": {
      "command": "npx",
      "args": ["@anthropic-ai/mcp-server-playwright"]
    }
  }
}
```

MCP tools are discovered at startup and integrated into the tool registry with a 10-second connection timeout.

## REST API & WebSocket

### REST Endpoints (`/api/v1/*`)

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/health` | Health check (simple) | No |
| GET | `/health/detailed` | Detailed health check (DB/Redis/LLM status) | Yes |
| GET | `/metrics` | Prometheus-format metrics | Yes |
| GET/PUT | `/api/v1/config` | Configuration read/update | Yes |
| GET | `/api/v1/tools` | List available tools | Yes |
| GET | `/api/v1/executions` | List executions (filterable, max 50) | Yes |
| GET | `/api/v1/executions/{id}` | Execution details | Yes |
| GET | `/api/v1/executions/{id}/replay` | Replay events for an execution | Yes |
| POST | `/api/v1/executions/{id}/rerun` | Re-run an execution | Yes |
| GET/POST/PUT/DELETE | `/api/v1/scheduler/tasks` | Scheduler task management | Yes |
| GET | `/api/v1/quota` | Provider quota/cost status | Yes |
| GET | `/api/v1/dev/sessions` | Active AI dev sessions (Claude, Gemini, Codex, Cursor) | Yes |
| GET | `/api/v1/dev/sessions/{tool}` | Sessions filtered by tool | Yes |
| GET/POST/DELETE | `/api/v1/pairing/*` | PIN-based device pairing | Yes |
| POST | `/api/v1/browser/*` | Browser control API | Yes |

### WebSocket Endpoints

| Path | Description |
|------|-------------|
| `/ws/chat` | Interactive chat |
| `/ws/events` | Event stream (real-time notifications) |
| `/ws/gateway` | Chrome extension gateway protocol |

### Security Middleware

All API endpoints are protected with configurable security layers:

- **Authentication**: HMAC signature, JWT token, or API key
- **Rate Limiting**: Per-IP and per-user request throttling
- **Approval Nonce**: One-time tokens for high-risk operations

## Chrome Extension (Browser Control)

Control your Chrome browser remotely via a lightweight extension that connects through the WebSocket gateway:

```
Chrome Extension ←→ /ws/gateway ←→ Cratos Server ←→ BrowserRelay ←→ Tools
```

Features:
- Tab management (list, open, close, activate)
- Page navigation and content extraction
- DOM interaction (click, type, screenshot)
- Bidirectional JSON-RPC communication

The `browser` tool automatically detects whether a Chrome extension is connected and falls back to MCP-based browser automation if not.

## Graph RAG Memory

Cross-session conversation memory powered by entity graph and hybrid vector search:

- **Turn Decomposition**: Breaks conversations into semantic units
- **Entity Extraction**: Rule-based named entity recognition
- **Graph Construction**: Entities linked by co-occurrence and relationships
- **Hybrid Search**: `embedding_similarity * 0.5 + proximity * 0.3 + entity_overlap * 0.2`

Data stored in `~/.cratos/memory.db` (SQLite) and `~/.cratos/vectors/memory` (HNSW index).

## TUI Chat

Interactive terminal-based chat interface:

```bash
cratos tui                    # Launch TUI
cratos tui --persona athena   # Start with specific persona
```

| Feature | Description |
|---------|-------------|
| **Markdown Rendering** | Rich text rendering via tui-markdown |
| **Mouse Scroll** | Scroll through conversation history |
| **Input History** | Up/Down arrow navigation (max 50 entries) |
| **Multi-Provider Quota** | Real-time quota display per provider |
| **Keyboard Shortcuts** | F2: toggle mouse, Ctrl+C: quit |

## Voice Control

Built-in voice assistant with Speech-to-Text, Text-to-Speech, and Voice Activity Detection:

```bash
cratos voice                  # Start voice assistant (Korean)
cratos voice --lang en        # English
cratos voice --lang ja        # Japanese
cratos voice --lang zh        # Chinese
```

| Component | Engine | Notes |
|-----------|--------|-------|
| **STT** | OpenAI Whisper API / Local Whisper (candle) | Local requires `local-stt` feature |
| **TTS** | Edge TTS | Free, no API key required |
| **VAD** | Silero VAD (ONNX) | Detects speech activity |

> Local Whisper STT: `cargo build --features local-stt` (downloads model on first run)

## Device Pairing

PIN-based device pairing for secure remote management:

```bash
cratos pair start             # Generate pairing PIN
cratos pair devices           # List paired devices
cratos pair unpair <device>   # Remove a paired device
```

Paired devices can control Cratos via REST API or WebSocket with device-level authentication.

## Proactive Scheduler

Schedule automated tasks:

| Trigger Type | Example | Description |
|--------------|---------|-------------|
| **Cron** | `0 9 * * *` | Daily at 9 AM |
| **Interval** | `{ seconds: 300, immediate: true }` | Every 5 minutes |
| **OneTime** | `{ at: "2026-03-01T10:00:00Z" }` | Single execution |
| **File** | `{ pattern: "*.json", action: "watch" }` | On file change |
| **System** | `{ metric: "cpu", threshold: 80 }` | On system event |

Task actions: `NaturalLanguage`, `ToolCall`, `Notification`, `Shell`, `Webhook`.

Manage via REST API (`/api/v1/scheduler/tasks`) or natural language.

## Testing

```bash
# Run all tests
cargo test --workspace

# Run integration tests only
cargo test --test integration_test

# Test specific crate
cargo test -p cratos-llm
cargo test -p cratos-tools
cargo test -p cratos-core
```

## Documentation

- [Setup Guide](./docs/en/SETUP_GUIDE.md) | [한국어](./docs/SETUP_GUIDE.md)
- [User Guide](./docs/en/USER_GUIDE.md) | [한국어](./docs/USER_GUIDE.md)
- [Developer Test Guide](./docs/en/TEST_GUIDE_DEV.md) | [한국어](./docs/TEST_GUIDE_DEV.md)
- [User Test Guide](./docs/en/TEST_GUIDE_USER.md) | [한국어](./docs/TEST_GUIDE_USER.md)

### Detailed Guides

| Guide | Description |
|-------|-------------|
| [Telegram](./docs/en/guides/TELEGRAM.md) | Telegram bot integration (teloxide) |
| [Slack](./docs/en/guides/SLACK.md) | Slack app integration (Socket Mode / Events API) |
| [Discord](./docs/en/guides/DISCORD.md) | Discord bot integration (serenity) |
| [WhatsApp](./docs/en/guides/WHATSAPP.md) | WhatsApp integration (Baileys / Business API) |
| [Browser Automation](./docs/en/guides/BROWSER_AUTOMATION.md) | MCP-based browser control + Chrome extension |
| [Skill Auto-Generation](./docs/en/guides/SKILL_AUTO_GENERATION.md) | Pattern learning and skill creation |
| [Graceful Shutdown](./docs/en/guides/GRACEFUL_SHUTDOWN.md) | 5-phase safe shutdown mechanism |
| [Live Canvas](./docs/en/guides/LIVE_CANVAS.md) | Real-time visual workspace |
| [Native Apps](./docs/en/guides/NATIVE_APPS.md) | Tauri desktop application |

## License

MIT

## Contributing

Issues and PRs welcome.

---

**Languages**: [English](./README.md) | [한국어](./README.ko.md)
