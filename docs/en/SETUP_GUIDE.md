# Cratos Setup Guide

Cratos is an AI assistant that runs on **your computer**, allowing you to remotely command your PC via Telegram even while you're away.

## Table of Contents

1. [Understanding the Concept](#1-understanding-the-concept)
2. [System Requirements](#2-system-requirements)
3. [Creating a Telegram Bot](#3-creating-a-telegram-bot)
4. [Getting LLM API Keys](#4-getting-llm-api-keys)
5. [Environment Variables](#5-environment-variables)
6. [Running Cratos](#6-running-cratos)
7. [Verifying Installation](#7-verifying-installation)
8. [Security Configuration](#8-security-configuration)
9. [Troubleshooting](#9-troubleshooting)
10. [Stopping and Restarting](#10-stopping-and-restarting)

---

## 1. Understanding the Concept

```
┌─────────────────────────────────────────────────────────────┐
│                   Your Computer (Home/Office)                │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                      Cratos                          │   │
│  │  - File read/write                                   │   │
│  │  - Command execution (Docker sandbox)                │   │
│  │  - Git/GitHub operations                             │   │
│  │  - Web information gathering                         │   │
│  │  - 8 LLM provider integrations                       │   │
│  └─────────────────────────────────────────────────────┘   │
│                           ↑                                 │
│                           │ Telegram API                    │
└───────────────────────────┼─────────────────────────────────┘
                            │
                            ↓
                   ┌─────────────────┐
                   │  Telegram Server │
                   └─────────────────┘
                            ↑
                            │
                   ┌─────────────────┐
                   │  Your Smartphone │
                   │   (Anywhere!)    │
                   └─────────────────┘
```

**Key Point**: Cratos runs on your computer. Through your personal Telegram bot, you can command your PC from anywhere.

---

## 2. System Requirements

| Item | Minimum | Recommended |
|------|---------|-------------|
| **OS** | macOS 10.15+, Windows 10+, Ubuntu 20.04+ | Latest version |
| **CPU** | 2 cores | 4+ cores |
| **RAM** | 4GB (runtime) / 8GB (build) | 8GB+ |
| **Disk** | 5GB | 10GB+ |
| **Rust** | 1.80+ | Latest stable |
| **Network** | Internet connection | Static IP or DDNS |

> **Note**: No Docker or PostgreSQL required! Data is automatically stored in `~/.cratos/cratos.db` (SQLite).

### For Ollama Local LLM

| Model | RAM | VRAM (GPU) | Description |
|-------|-----|------------|-------------|
| Llama 3.2 1B | 2GB | 2GB | Lightweight, fast |
| Llama 3.2 3B | 4GB | 4GB | Balanced |
| Qwen 2.5 7B | 8GB | 8GB | High quality |
| Llama 3.1 70B | 48GB | 48GB | Best quality |

> **Note**: No GPU required when using external LLM APIs (OpenAI, Novita, etc.)!

### Installing Rust (Required)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify installation
rustc --version  # Requires 1.80+
```

### Installing Docker (Optional - for Sandbox)

Docker is only used for isolated execution of dangerous commands. Basic features work without it.

**macOS**:
```bash
brew install --cask docker
```

**Windows**:
- Download and install [Docker Desktop](https://www.docker.com/products/docker-desktop/)

**Linux**:
```bash
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER
```

---

## 3. Creating a Telegram Bot

You need to create your own Telegram bot. **It takes about 5 minutes.**

### 3.1 Create Bot with BotFather

1. Open Telegram app (phone or desktop)
2. Search for `@BotFather`
3. Select the official BotFather with blue checkmark
4. Type `/newbot`

### 3.2 Choose Bot Name

```
BotFather: Alright, a new bot. How are we going to call it?
You: My Personal Assistant
```

Enter the display name for your bot.

### 3.3 Choose Bot Username

```
BotFather: Good. Now let's choose a username for your bot.
You: my_personal_cratos_bot
```

**Important**: Must end with `_bot`.

### 3.4 Copy Token

```
BotFather: Done! Congratulations on your new bot.
Use this token to access the HTTP API:
7123456789:AAHxxxxxxxxxxxxxxxxxxxxxxxxxx
```

Copy this token and keep it safe.

⚠️ **Warning**: Never share this token publicly!

---

## 4. Getting LLM API Keys

Cratos needs LLM API keys for AI functionality.

### 💰 Paid Options

#### OpenAI (GPT-5.2)
1. Visit https://platform.openai.com/api-keys
2. Create account or sign in
3. Click "Create new secret key"
4. Copy key (e.g., `sk-proj-xxxx...`)

#### Anthropic (Claude)
1. Visit https://console.anthropic.com/settings/keys
2. Create account or sign in
3. Click "Create Key"
4. Copy key (e.g., `sk-ant-api03-xxxx...`)

#### ZhipuAI (GLM)
1. Visit https://open.bigmodel.cn
2. Create account and get API key
3. Copy key

#### Alibaba (Qwen)
1. Visit https://dashscope.console.aliyun.com
2. Create account and get API key
3. Copy key

### 🆓 Free Options

#### OpenRouter (Recommended!)
1. Visit https://openrouter.ai
2. Sign up with GitHub/Google (simple!)
3. Get API key from API Keys menu
4. **Free models**: Qwen3-32B, Llama 3.2 (1000 calls/day)

#### Novita AI (Free signup)
1. Visit https://novita.ai
2. Free signup
3. Get API key
4. **Free models**: Llama 3.2, Qwen2.5-7B, GLM-4-9B

#### Ollama (Completely free, local)
Use locally for free without API key:
```bash
# Install Ollama (macOS)
brew install ollama

# Download model
ollama pull llama3.2

# Run Ollama
ollama serve
```

---

## 5. Environment Variables

### 5.1 Download Cratos

```bash
git clone https://github.com/cratos/cratos.git
cd cratos
```

### 5.2 Create Configuration File

```bash
cp .env.example .env
```

### 5.3 Edit .env File

```bash
# Open with text editor
nano .env
# or
code .env
```

Only fill in the required fields:

```bash
# ================================
# Required: Telegram Bot Token
# ================================
TELEGRAM_BOT_TOKEN=7123456789:AAHxxxxxxxxxxxxxxxxxxxxxxxxxx

# ================================
# LLM API Key (choose at least one)
# ================================

# Paid: OpenAI
OPENAI_API_KEY=sk-proj-your-key-here

# Paid: Anthropic
ANTHROPIC_API_KEY=sk-ant-api03-your-key-here

# Paid: ZhipuAI GLM
BIGMODEL_API_KEY=your-bigmodel-key-here

# Paid: Alibaba Qwen
DASHSCOPE_API_KEY=your-dashscope-key-here

# Free: OpenRouter (Recommended!)
OPENROUTER_API_KEY=sk-or-your-key-here

# Free: Novita AI
NOVITA_API_KEY=your-novita-key-here

# Free: Ollama (no key needed, uncomment below)
# OLLAMA_BASE_URL=http://host.docker.internal:11434

# ================================
# Optional (defaults available)
# ================================
# REDIS_URL=redis://localhost:6379   # Uses memory session if not set
# CRATOS_DATA_DIR=~/.cratos          # Data storage path
RUST_LOG=cratos=info,tower_http=info
```

> **Note**: `DATABASE_URL` is no longer needed. Uses embedded SQLite (`~/.cratos/cratos.db`).

### 💡 Cost-Saving Tips

To start for free:
1. Get **OpenRouter** key only (1-minute signup with GitHub)
2. Set only `OPENROUTER_API_KEY` in `.env`
3. 1000 free calls per day!

---

## 6. Running Cratos

```bash
# Build (first run takes ~5-10 minutes)
cargo build --release

# Run
cargo run --release

# Or in one command
cargo run
```

On successful start, you'll see:
```
Starting Cratos AI Assistant v0.1.0
Configuration loaded
Data directory: /Users/yourname/.cratos
SQLite event store initialized at /Users/yourname/.cratos/cratos.db
LLM provider initialized: anthropic
Tool registry initialized with 11 tools
Telegram adapter started
HTTP server listening on http://127.0.0.1:9742
```

> **Note**: Database file (`~/.cratos/cratos.db`) is created automatically.

---

## 7. Verifying Installation

### 7.1 Health Check

```bash
curl http://localhost:9742/health
```

Response:
```json
{"status":"healthy","version":"0.1.0"}
```

### 7.2 Test via Telegram

1. Open Telegram app
2. Search for your bot username (e.g., `@my_personal_cratos_bot`)
3. Select bot and click "Start" button
4. Send message: `Hello`

If you get a response within 10 seconds, it's working!

### 7.3 Basic Command Tests

```
You: Show me the files in current directory
Bot: (file list response)

You: What's today's date?
Bot: (date response)
```

---

## 8. Security Configuration

Cratos provides several built-in security features.

### 8.1 Sandbox Settings

Create `config/local.toml`:

```toml
[security]
# strict: isolate all tools
# moderate: isolate only risky tools (default)
# disabled: development mode
sandbox_policy = "moderate"

[security.sandbox]
default_network = "none"    # Block network
max_memory_mb = 512         # Memory limit
max_cpu_percent = 50        # CPU limit
```

### 8.2 Credential Security

Store API keys more securely in OS keychain:

```toml
[security]
# auto: auto-select based on platform
# keychain: macOS Keychain
# secret_service: Linux
# encrypted_file: Encrypted file
credential_backend = "auto"
```

### 8.3 Prompt Injection Defense

Automatically blocks malicious prompt attacks:

```toml
[security.injection]
# Block threshold: info, low, medium, high, critical
block_threshold = "medium"
```

---

## 9. Troubleshooting

### Bot Not Responding

```bash
# 1. Check logs
docker-compose logs cratos

# 2. Check container status
docker-compose ps

# 3. Restart
docker-compose restart cratos
```

### "Unauthorized" or API Key Error

1. Check API key in `.env` file
2. Remove leading/trailing whitespace
3. Restart: `docker-compose restart cratos`

### Port Conflict

If port conflicts with another program:

```yaml
# Edit docker-compose.yml
ports:
  - "9999:8080"  # Use different port instead of 9742
```

### Database Errors

SQLite is embedded, no separate setup needed. If issues occur:

```bash
# Check data directory
ls -la ~/.cratos/

# Check database file
sqlite3 ~/.cratos/cratos.db ".tables"

# Reset (deletes data)
rm ~/.cratos/cratos.db
```

### Out of Memory (with Ollama)

Use a smaller model:
```bash
ollama pull llama3.2:1b   # 1B model (2GB RAM)
```

---

## 10. Stopping and Restarting

### Stop

Press `Ctrl+C` in terminal, or:

```bash
# Find and kill process
pkill -f "cratos"
```

### Restart

```bash
cargo run --release
```

### Reset (Delete All Data)

```bash
rm -rf ~/.cratos/
```

### Background Execution (Optional)

```bash
# Using nohup
nohup cargo run --release > cratos.log 2>&1 &

# Or register as systemd service (Linux)
```

---

## Next Steps

Installation complete! Check the [User Guide](./USER_GUIDE.md) for various features.

### Recommended First Use

```
You: Hi, what can you do?
```

Cratos will tell you about its capabilities.
