# Cratos Test Guide - Developer Edition

## Test Objectives

1. Verify the `init` command works correctly
2. Verify multilingual support functions properly
3. Validate the installation scripts
4. Validate the release.yml workflow

---

## 1. Basic Build & Test

```bash
# Run all tests
cargo test --workspace

# Verify build
cargo build --release

# Check CLI help
cargo run -- --help
cargo run -- init --help
```

**Checklist:**
- [ ] All tests pass
- [ ] Release build succeeds
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

## 6. Integration Test

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

## 7. Edge Cases

### 7.1 Empty Input Handling

```bash
cargo run -- init --lang en
# Enter empty value for API key -> should re-prompt
```

### 7.2 Ctrl+C Handling

```bash
cargo run -- init --lang en
# Press Ctrl+C mid-way -> should exit cleanly
```

### 7.3 Invalid Language Code

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
