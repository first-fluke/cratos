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
- **Multi-LLM Support**: OpenAI, Anthropic, Gemini, DeepSeek, Groq, Fireworks, SiliconFlow, GLM, Qwen, Moonshot, Novita, OpenRouter, Ollama (13 providers)
- **Smart Routing**: Automatic model selection by task type reduces costs by 70%
- **Free Model Support**: Free LLMs via OpenRouter, Novita (Llama, Qwen, GLM)
- **Replay Engine**: All executions stored as events, timeline view and replay
- **Tool System**: 15 built-in tools (file, HTTP, Git, GitHub, command execution, browser, WoL, config)
- **Channel Adapters**: Telegram, Slack, Discord, Matrix support
- **Security**: Docker sandbox, credential encryption, prompt injection defense
- **Olympus OS**: Mythology-based 3-layer agent organization (Pantheon/Decrees/Chronicles)

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
curl http://localhost:8090/health
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
│   ├── cratos-core/      # Orchestration engine, security, credentials
│   ├── cratos-channels/  # Channel adapters (Telegram, Slack, Discord, Matrix)
│   ├── cratos-tools/     # Tool registry, sandbox
│   ├── cratos-llm/       # LLM providers, token counting, embeddings
│   ├── cratos-replay/    # Event logging and replay (SQLite)
│   ├── cratos-skills/    # Automatic skill generation system
│   ├── cratos-search/    # Vector search, semantic indexing
│   ├── cratos-audio/     # Voice control (STT/TTS, optional)
│   └── cratos-canvas/    # Canvas (future)
├── config/
│   ├── default.toml      # Default configuration
│   ├── pantheon/         # Persona TOML files (14 personas: 5 core + 9 extended)
│   └── decrees/          # Laws, ranks, development rules
├── src/
│   ├── main.rs           # Application entry point
│   ├── cli/              # CLI commands (init, doctor, quota, tui, pantheon, decrees, chronicle)
│   ├── api/              # REST API (config, tools, executions, scheduler, quota)
│   ├── websocket/        # WebSocket handlers (chat, events)
│   └── server.rs         # Server initialization

~/.cratos/                # Data directory (auto-created)
├── cratos.db             # SQLite main DB (events, execution history)
├── skills.db             # SQLite skills DB (skills, patterns)
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
| `GOOGLE_API_KEY` | Google Gemini API key | |
| `ZHIPU_API_KEY` | ZhipuAI GLM API key | |
| `DASHSCOPE_API_KEY` | Alibaba Qwen API key | |
| `OPENROUTER_API_KEY` | OpenRouter API key | |
| `NOVITA_API_KEY` | Novita AI API key (free) | |

> **Note**: `DATABASE_URL` is no longer needed. Uses embedded SQLite.

### Configuration Files

Default settings are in `config/default.toml`. Create `config/local.toml` to customize for your local environment.

## LLM Providers

### Paid Providers

| Provider | Models | Features |
|----------|--------|----------|
| **OpenAI** | GPT-5.2, GPT-5.1, GPT-5 | Latest generation, coding |
| **Anthropic** | Claude Sonnet 4.5, Claude Haiku 4.5, Claude Opus 4.5 | Excellent code generation |
| **Gemini** | Gemini 3 Pro, Gemini 3 Flash, Gemini 2.5 Pro | Long context, multimodal |
| **GLM** | GLM-4.7, GLM-4-Flash | ZhipuAI models |
| **Qwen** | Qwen3-Max, Qwen3-Plus, Qwen3-Flash, Qwen3-Coder | Multilingual, coding, reasoning |
| **DeepSeek** | DeepSeek-V3.2, DeepSeek-R1 | Ultra low cost, reasoning |

### Free/Low-Cost Providers

| Provider | Models | Limits |
|----------|--------|--------|
| **OpenRouter** | Qwen3-Max, Llama 3.3 70B, Gemma 3 27B | 1000/day |
| **Novita** | Qwen3-Plus, GLM-4-9B, Llama 3.3 70B | Free signup |
| **Groq** | Llama 3.3 70B, Mixtral 8x7B | Free, ultra-fast inference |
| **Ollama** | All local models | Unlimited (hardware dependent) |

### Model Routing

Automatic model selection based on task type:

| Task Type | Model Tier | Example Models |
|-----------|------------|----------------|
| Classification | Fast | GPT-5.2-mini, Claude Haiku |
| Summarization | Fast | GPT-5.2-mini, Gemini Flash |
| Conversation | Standard | GPT-5.2, Claude Sonnet |
| CodeGeneration | Standard | GPT-5.2, Claude Sonnet |
| Planning | Premium | GPT-5.2-turbo, Claude Opus |

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
cratos tui                        # Launch interactive TUI chat

# Pantheon (Personas)
cratos pantheon list              # List personas
cratos pantheon show sindri       # Show persona details
cratos pantheon summon sindri     # Summon (activate) a persona
cratos pantheon dismiss           # Dismiss active persona

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

| Tool | Description | Risk Level |
|------|-------------|------------|
| `file_read` | Read files | Low |
| `file_write` | Write files | Medium |
| `file_list` | List directory | Low |
| `http_get` | HTTP GET request | Low |
| `http_post` | HTTP POST request | Medium |
| `exec` | Command execution (sandboxed) | High |
| `git_status` | Git status check | Low |
| `git_commit` | Git commit creation | Medium |
| `git_branch` | Git branch management | Medium |
| `git_diff` | Git diff check | Low |
| `git_push` | Git push to remote | High |
| `github_api` | GitHub API integration | Medium |
| `browser` | Browser automation | Medium |
| `wol` | Wake-on-LAN | Medium |
| `config` | Natural language configuration | Medium |

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

- [Setup Guide](./docs/SETUP_GUIDE.md) - For first-time users
- [User Guide](./docs/USER_GUIDE.md) - Feature usage
- [PRD](./PRD.md) - Detailed requirements

## License

MIT

## Contributing

Issues and PRs welcome.

---

**Languages**: [English](./README.md) | [한국어](./README.ko.md)
