# Cratos - AI-Powered Personal Assistant

Cratos is a **Rust-based AI assistant** that understands natural language commands from Telegram/Slack, gathers information, executes tasks, and reports results.

## Key Features

- **No Docker Required**: Runs immediately with embedded SQLite (`~/.cratos/cratos.db`)
- **Automatic Skill Generation**: Learns usage patterns to automatically create workflow skills
- **Multi-LLM Support**: OpenAI, Anthropic, Gemini, Ollama, GLM, Qwen, OpenRouter, Novita, Groq, DeepSeek
- **Smart Routing**: Automatic model selection by task type reduces costs by 70%
- **Free Model Support**: Free LLMs via OpenRouter, Novita (Llama, Qwen, GLM)
- **Replay Engine**: All executions stored as events, timeline view and replay
- **Tool System**: 11 built-in tools (file, HTTP, Git, GitHub, command execution)
- **Channel Adapters**: Telegram, Slack, Discord, Matrix support
- **Security**: Docker sandbox, credential encryption, prompt injection defense

## System Requirements

| Item | Minimum¹ | Recommended | Optimal |
|------|----------|-------------|---------|
| **OS** | macOS 11+, Windows 10, Ubuntu 20.04+ | macOS 12+, Windows 10+, Ubuntu 22.04+ | Latest |
| **CPU** | 1 core | 1 core | 2+ cores |
| **RAM** | 256MB (runtime) / 2GB (build) | 1GB (runtime) / 4GB (build) | 4GB+ |
| **Disk** | 100MB | 1GB | 5GB+ |
| **Rust** | 1.80+ | 1.80+ | Latest stable |
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

### 1. Environment Setup

```bash
# Create environment file
cp .env.example .env

# Edit .env file (add your API keys)
vim .env
```

### 2. Run (No Docker Required!)

```bash
# Build and run
cargo build --release
cargo run --release

# Or at once
cargo run

# Health check
curl http://localhost:9742/health
```

Data is automatically stored in `~/.cratos/cratos.db`.

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
├── config/               # Configuration files
└── src/main.rs           # Application entry point

~/.cratos/                # Data directory (auto-created)
├── cratos.db             # SQLite main DB (events, execution history)
└── skills.db             # SQLite skills DB (skills, patterns)
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
| `BIGMODEL_API_KEY` | ZhipuAI GLM API key | |
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
| **GLM** | GLM-4.7, GLM-4-Plus, GLM-4-Air | Chinese optimized |
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
| Classification | Fast | GPT-4o-mini, Claude Haiku |
| Summarization | Fast | GPT-4o-mini, Gemini Flash |
| Conversation | Standard | GPT-4o, Claude Sonnet |
| CodeGeneration | Standard | GPT-4o, Claude Sonnet |
| Planning | Premium | GPT-4-turbo, Claude Opus |

## Security Features

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
| `github_api` | GitHub API integration | Medium |

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
