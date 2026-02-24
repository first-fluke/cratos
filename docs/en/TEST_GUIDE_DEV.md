# Cratos Test Guide - Developer Edition

## Test Objectives

1. Verify the full automated test suite passes (1,286+ tests)
2. Verify the `init` command works correctly
3. Verify multilingual support functions properly
4. Validate the installation scripts
5. Validate the release.yml CI workflow

---

## 1. Basic Build & Test

```bash
# Run all tests (1,286+ tests)
cargo test --workspace

# Quick type check
cargo check --all-targets

# Lint
cargo clippy --all-targets

# Build (daily development)
cargo build --profile dev-release -p cratos

# Release build (deployment, ~10 min)
cargo build --release

# Check CLI help
cargo run -- --help
cargo run -- init --help
```

**Checklist:**
- [ ] All tests pass (1,286+ tests, 0 failures)
- [ ] No clippy warnings
- [ ] Build succeeds
- [ ] `init` command appears in help
- [ ] `--lang` option is displayed

---

## 2. Wizard Feature Tests

### 2.1 Language Detection

```bash
# Force English
cargo run -- init --lang en

# Force Korean
cargo run -- init --lang ko

# System language detection (based on LANG environment variable)
LANG=ko_KR.UTF-8 cargo run -- init
LANG=en_US.UTF-8 cargo run -- init
```

**Checklist:**
- [ ] `--lang en` produces English output
- [ ] `--lang ko` produces Korean output
- [ ] `LANG=ko_KR` auto-detects Korean
- [ ] `LANG=en_US` auto-detects English

### 2.2 Existing .env File Handling

```bash
# Run with an existing .env file
echo "TEST=1" > .env
cargo run -- init --lang en
# Should prompt "Overwrite?"
```

**Checklist:**
- [ ] Overwrite confirmation prompt appears when .env exists
- [ ] Selecting "No" shows cancellation message
- [ ] Selecting "Yes" proceeds normally

### 2.3 Telegram Skip

```bash
cargo run -- init --lang en
# At Step 1, select "Skip Telegram setup?" -> Yes
```

**Checklist:**
- [ ] Proceeds to Step 2 after skipping
- [ ] Final .env contains `# TELEGRAM_BOT_TOKEN=` (commented out)

### 2.4 Provider Selection

Verify each provider after selection:

| Provider | Expected env_var | Check |
|----------|------------------|-------|
| OpenRouter | `OPENROUTER_API_KEY` | [ ] |
| Groq | `GROQ_API_KEY` | [ ] |
| Google AI | `GEMINI_API_KEY` (or `GOOGLE_API_KEY`) | [ ] |
| OpenAI | `OPENAI_API_KEY` | [ ] |
| Anthropic | `ANTHROPIC_API_KEY` | [ ] |
| DeepSeek | `DEEPSEEK_API_KEY` | [ ] |
| Ollama | `OLLAMA_BASE_URL` | [ ] |

### 2.5 Connection Test Logic

```bash
# Ollama test (with Ollama running)
ollama serve &
cargo run -- init --lang en
# Select Ollama -> should connect successfully

# Ollama test (without Ollama running)
pkill ollama
cargo run -- init --lang en
# Select Ollama -> should fail + show "Continue anyway?" prompt
```

**Checklist:**
- [ ] Ollama running -> connection succeeds
- [ ] Ollama not running -> failure message + option to continue

### 2.6 Telegram Token Validation

```bash
cargo run -- init --lang en
# Enter a valid token -> success
# Enter an invalid token -> failure + "Continue anyway?"
```

**Checklist:**
- [ ] Valid token -> "Success!"
- [ ] Invalid token -> "Failed" + continue option

---

## 3. Installation Script Tests

### 3.1 install.sh Syntax Validation

```bash
# Lint with shellcheck
shellcheck scripts/install.sh

# Check execution permissions
ls -la scripts/install.sh
# Should be -rwxr-xr-x
```

### 3.2 install.sh Dry Run

```bash
# Review script contents (without executing)
cat scripts/install.sh

# Test platform detection functions
bash -c 'source scripts/install.sh; detect_os'
bash -c 'source scripts/install.sh; detect_arch'
bash -c 'source scripts/install.sh; get_target'
```

### 3.3 install.ps1 Syntax Validation (Windows or PowerShell Core)

```powershell
# Syntax check in PowerShell
$script = Get-Content scripts/install.ps1 -Raw
[System.Management.Automation.PSParser]::Tokenize($script, [ref]$null)
```

---

## 4. Release Workflow Validation

### 4.1 YAML Syntax Check

```bash
# Install yamllint (brew install yamllint)
yamllint .github/workflows/release.yml
```

### 4.2 Build Matrix Verification

Verify the following targets in `release.yml`:

| Target | OS Runner | Check |
|--------|-----------|-------|
| `x86_64-apple-darwin` | `macos-13` | [ ] |
| `aarch64-apple-darwin` | `macos-14` | [ ] |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | [ ] |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` + cross | [ ] |
| `x86_64-pc-windows-msvc` | `windows-latest` | [ ] |

### 4.3 Local Cross-Build Test (Optional)

```bash
# Build for current platform
cargo build --release

# Build for another target (requires rustup target add)
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

---

## 5. Generated .env File Validation

```bash
# After completing init
cat .env

# Expected structure:
# - LLM Provider section
# - CRATOS_LLM__DEFAULT_PROVIDER setting
# - Telegram section
# - Server section (HOST, PORT)
# - Logging section (RUST_LOG)
# - Default Persona section
```

**Checklist:**
- [ ] Appropriate comments are included
- [ ] Sections are clearly separated
- [ ] Sensitive information is stored correctly

---

## 6. Automated Test Structure

### 6.1 Test Category Overview

| Category | Location | Description |
|----------|----------|-------------|
| **Tool Registry** | `crates/cratos-tools/src/builtins/mod.rs` | 23 built-in tool registration and count verification |
| **Individual Tools** | `crates/cratos-tools/src/builtins/*.rs` | Per-tool definition, input validation, security tests |
| **Orchestrator** | `crates/cratos-core/src/orchestrator/tests.rs` | Config, input, error sanitization, tool refusal heuristics |
| **Sanitize** | `crates/cratos-core/src/orchestrator/sanitize.rs` | `is_tool_refusal`, `is_fake_tool_use_text`, error sanitization |
| **Integration Tests** | `tests/integration_test.rs` | Cross-crate integration (LLM, tools, replay, channels) |
| **LLM Providers** | `crates/cratos-llm/src/` | Model tiers, routing rules, provider config |
| **Replay** | `crates/cratos-replay/src/` | Event store, execution lifecycle |
| **Skills** | `crates/cratos-skills/src/` | Skill generation, routing, registry |
| **Memory** | `crates/cratos-memory/src/` | Graph RAG, conversation memory |
| **Security** | `crates/cratos-core/src/security/` | Rate limiter, circuit breaker |

### 6.2 Built-in Tools List (23 tools)

The full tool list verified by integration tests (`tests/integration_test.rs`):

```
file_read, file_write, file_list, http_get, http_post, exec, bash,
git_status, git_commit, git_branch, git_diff, git_push, git_clone, git_log,
github_api, browser, wol, config, web_search, agent_cli,
send_file, image_generate, app_control
```

> **Important**: When adding/removing tools, 3 locations must be kept in sync:
> 1. `crates/cratos-tools/src/builtins/mod.rs` — registration + test count
> 2. `tests/integration_test.rs` — `expected_tools` array and count
> 3. Per-tool test file

### 6.3 Running Specific Crate/Module Tests

```bash
# Tool registry tests only
cargo test -p cratos-tools

# Orchestrator tests only
cargo test -p cratos-core

# Integration tests only
cargo test --test integration_test

# Specific test functions
cargo test test_tool_registry_with_builtins
cargo test test_tool_refusal
cargo test test_fake_tool_use_detection
```

### 6.4 Orchestrator (ReAct Loop) Tests

The Workflow Engine has been removed and replaced by an autonomous ReAct loop. Related tests:

| Test | Verifies |
|------|----------|
| `test_tool_refusal_*` | Detects when LLM returns short text without tool calls |
| `test_fake_tool_use_detection` | Detects fake tool-use text like `[Used 1 tool: browser:OK]` |
| `test_sanitize_error_for_user` | Masks sensitive info such as file paths |
| `test_sanitize_for_session_memory` | Prevents prompt injection |
| `test_orchestrator_config_failure_limits` | Consecutive/total failure limit settings |
| `test_max_execution_secs_default` | Execution timeout default (180s) |

> **Note**: The `is_tool_refusal` function lives in `sanitize.rs`, but its tests are in `orchestrator/tests.rs`.

### 6.5 app_control Tool Tests

`app_control` is a macOS AppleScript/JXA automation tool, classified as `RiskLevel::High`.

```bash
# app_control tests
cargo test -p cratos-tools app_control
```

Test items:
- Tool definition (name, description, parameter schema)
- Security validation (`BLOCKED_PATTERNS`: blocks `do shell script`, `System Preferences`, `password`, etc.)

### 6.6 Integration Test Details

`tests/integration_test.rs` verifies cross-crate integration:

- **LLM Router**: Provider config, routing rules, per-tier default models
- **Tool Registry**: 23 built-in tool registration, schema validation
- **Replay**: Execution lifecycle, event types, status transitions
- **Orchestrator**: Input creation, session keys, configuration
- **Channels**: Message normalization (Telegram, Slack)
- **Security**: Rate limiter, circuit breaker, metrics

---

## 7. E2E Integration Test

```bash
# 1. Start from a clean state
rm -f .env

# 2. Configure with init
cargo run -- init --lang ko

# 3. Verify with doctor
cargo run -- doctor

# 4. Start the server (Ctrl+C to stop)
cargo run -- serve
```

**Checklist:**
- [ ] init -> .env created
- [ ] doctor -> configuration verified
- [ ] serve -> server starts successfully

---

## 8. Edge Cases

### 8.1 Empty Input Handling

```bash
cargo run -- init --lang en
# Enter empty value for API key -> should re-prompt
```

### 8.2 Ctrl+C Handling

```bash
cargo run -- init --lang en
# Press Ctrl+C mid-way -> should exit cleanly
```

### 8.3 Invalid Language Code

```bash
cargo run -- init --lang fr
# -> Should fall back to English (default)
```

---

## Bug Report Template

```
## Environment
- OS:
- Rust version:
- Command:

## Expected Behavior

## Actual Behavior

## Reproduction Steps
1.
2.
3.

## Logs/Screenshots
```
