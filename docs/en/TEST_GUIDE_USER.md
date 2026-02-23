# Cratos Test Guide - Non-Developer Edition

## Test Objective

Verify that non-developers can install with a single terminal command and complete the setup by following the wizard.

---

## Prerequisites

1. **Telegram account** required (used to create a bot)
2. Open a **Terminal/PowerShell**
   - macOS: Search "Terminal" in Spotlight
   - Windows: Search "PowerShell" in the Start menu

---

## Test Scenarios

### Scenario 1: One-Click Install (After Release)

> Note: Skip this test if no GitHub Release is available yet.

**macOS/Linux:**
```bash
curl -sSL https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.sh | sh
```

**Windows PowerShell:**
```powershell
irm https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.ps1 | iex
```

**Checklist:**
- [ ] Does the download proceed automatically?
- [ ] Is an installation complete message displayed?
- [ ] Does the wizard start automatically?

---

### Scenario 2: Running the Wizard Directly (Development Build)

From the project folder:

```bash
# Korean wizard
cargo run -- init --lang ko

# Or English wizard
cargo run -- init --lang en

# Or auto-detect system language
cargo run -- init
```

---

## Wizard Test Checklist

### Step 1: Welcome Screen

- [ ] Is a welcome message displayed?
- [ ] Is the 3-step description shown?
- [ ] Is the estimated time (~8 minutes) displayed?

### Step 2: Telegram Bot Setup

- [ ] Is the BotFather link (`https://t.me/BotFather`) displayed?
- [ ] Are the step-by-step instructions easy to understand?
- [ ] Is a "Skip" option available?

**Test A: Enter a Real Bot Token**
1. Click the link -> Telegram app opens
2. Send `/newbot` to BotFather
3. Enter bot name and username
4. Copy and paste the received token

**Test B: Skip**
1. Select "Yes" at "Skip Telegram setup?"

- [ ] Is the token input masked? (******* format)
- [ ] Does skipping work correctly?

### Step 3: AI Model Selection

- [ ] Are free/paid options clearly separated?
- [ ] Does each option include pricing information?
- [ ] Can you select with arrow keys?

**Recommended Test Order:**
1. **Groq** (free, easiest) - https://console.groq.com/keys
2. **OpenRouter** (free) - https://openrouter.ai/keys
3. **Google AI** (free) - https://aistudio.google.com/apikey

### Step 4: API Key Input

- [ ] Is the signup link displayed?
- [ ] Are step-by-step instructions provided?
- [ ] Is the API key masked?

### Step 5: Connection Test

- [ ] Does the Telegram connection test run? (if token was entered)
- [ ] Does the LLM connection test run?
- [ ] Are success/failure messages clear?
- [ ] Is there a "Continue anyway?" option on failure?

### Step 6: Completion

- [ ] Is a completion message displayed?
- [ ] Is a summary shown?
- [ ] Are next-step instructions provided?
- [ ] Was a `.env` file created?

---

## Verify Generated .env File

```bash
cat .env
```

**Checklist:**
- [ ] Is the selected LLM provider key saved?
- [ ] Is the Telegram token saved? (if entered)
- [ ] Is `CRATOS_LLM__DEFAULT_PROVIDER` set correctly?

---

## Troubleshooting

### Links Are Not Clickable
- Check "Enable URL clicking" in your terminal app settings
- Or manually copy the link and paste it into your browser

### Korean Characters Are Garbled
- Set your terminal encoding to UTF-8

### Cannot Enter API Key
- Paste: Ctrl+V (Windows) or Cmd+V (macOS)
- Press Enter after input

---

## Feedback Questions

After testing, please answer the following:

1. Were the instructions easy to understand? (1-5)
2. Were the links easy to find? (1-5)
3. Did you get stuck anywhere? (If so, where?)
4. Any suggestions for improvement?
