# Browser Automation - Autonomous Browser Control

## Overview

Cratos performs web automation through autonomous LLM-driven browser control. Without pre-coded workflows, the AI reads pages and decides the next action on its own.

### Browser Backends

| Backend | Connection | Features |
|---------|-----------|----------|
| **Chrome Extension** (default) | WebSocket to Cratos server | Controls the user's real browser, tab listing |
| **MCP (Playwright)** | JSON-RPC over stdio | Headless browser, cross-browser support |
| **Auto** (recommended) | Extension first, MCP fallback | Uses extension if connected, otherwise falls back to MCP |

### Core Features

| Feature | Description |
|---------|-------------|
| **Autonomous Control** | LLM reads pages and decides next actions (Plan-Act-Reflect) |
| **Site Search** | `search` action auto-generates search URLs for Naver Shopping, Coupang, Google, YouTube, etc. |
| **Text Click** | `click_text` clicks elements by visible text without CSS selectors |
| **Tab Management** | `get_tabs` lists open browser tabs (Chrome Extension only) |
| **Page Analysis** | `get_text`, `get_html` read page content (auto-truncated) |
| **Screenshots** | Full page or element capture |
| **Form Automation** | `fill`, `type`, `select`, `check` for form input |
| **JS Execution** | `evaluate` runs arbitrary JavaScript |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Cratos Orchestrator                       │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              LLM (Autonomous Agent)                      │ │
│  │    Analyze request → Select tool → Interpret → Repeat    │ │
│  └────────────────────────┬────────────────────────────────┘ │
│                           │ tool_call: browser               │
│  ┌────────────────────────▼────────────────────────────────┐ │
│  │                   BrowserTool                            │ │
│  │  ┌──────────────────┐  ┌──────────────────┐             │ │
│  │  │ Chrome Extension │  │ MCP Client       │             │ │
│  │  │ (WebSocket relay)│  │ (Playwright)     │             │ │
│  │  └────────┬─────────┘  └────────┬─────────┘             │ │
│  └───────────│─────────────────────│───────────────────────┘ │
└──────────────│─────────────────────│─────────────────────────┘
               │                     │
               ▼                     ▼
┌──────────────────────┐  ┌──────────────────────┐
│  User's Chrome       │  │  Headless Browser    │
│  (Extension installed)│  │  (Playwright MCP)    │
└──────────────────────┘  └──────────────────────┘
```

## Browser Actions

Cratos provides a single `browser` tool with an `action` parameter supporting diverse actions:

### Search & Navigation

| Action | Description | Required Parameters |
|--------|-------------|---------------------|
| `search` | Search on a known site (auto-generates URL) | `site`, `query` |
| `navigate` | Navigate to URL | `url` |
| `go_back` | Go back in history | - |
| `go_forward` | Go forward in history | - |
| `reload` | Reload page | - |
| `get_url` | Get current URL | - |
| `get_title` | Get page title | - |
| `get_tabs` | List open tabs (Chrome Extension only) | - |

### Element Interaction

| Action | Description | Required Parameters |
|--------|-------------|---------------------|
| `click` | Click by CSS selector | `selector` |
| `click_text` | Click by visible text on screen | `text` |
| `type` | Type text (appends to existing) | `selector`, `text` |
| `fill` | Fill form field (clears first) | `selector`, `value` |
| `select` | Select dropdown option | `selector`, `value` |
| `check` | Check/uncheck checkbox | `selector` |
| `hover` | Mouse hover | `selector` |
| `press` | Press keyboard key | `key` |
| `scroll` | Scroll page or element | `x`, `y` |

### Information Extraction

| Action | Description | Required Parameters |
|--------|-------------|---------------------|
| `get_text` | Extract text (omit selector for full page) | - |
| `get_html` | Extract HTML | - |
| `get_attribute` | Get element attribute | `selector`, `attribute` |
| `screenshot` | Capture screenshot | - |

### Wait & Advanced

| Action | Description | Required Parameters |
|--------|-------------|---------------------|
| `wait_for_selector` | Wait for element to appear | `selector` |
| `wait_for_navigation` | Wait for page load | - |
| `evaluate` | Execute JavaScript | `script` |
| `close` | Close browser | - |

### Supported Search Sites

The `site` parameter for the `search` action:

| Site | Identifier | Korean Alias |
|------|-----------|-------------|
| Naver Shopping | `naver_shopping` | `네이버쇼핑` |
| Naver | `naver` | `네이버` |
| Coupang | `coupang` | `쿠팡` |
| Google | `google` | `구글` |
| YouTube | `youtube` | `유튜브` |
| Amazon | `amazon` | `아마존` |
| Google Maps | `google_maps` | `구글맵` |

Unknown sites automatically fall back to `site:` Google search.

## MCP Server Setup

### 1. Playwright MCP (Recommended)

```bash
# Install
npm install -g @anthropic-ai/mcp-server-playwright

# Or run directly with npx
npx @anthropic-ai/mcp-server-playwright
```

### 2. MCP Configuration File

```json
// .mcp.json (project root)
{
  "mcpServers": {
    "playwright": {
      "command": "npx",
      "args": ["@anthropic-ai/mcp-server-playwright"],
      "env": {
        "BROWSER_TYPE": "chromium",
        "HEADLESS": "true"
      }
    }
  }
}
```

### 3. Chrome Extension Setup

```bash
# Check extension install path
cratos browser extension path

# Install extension
cratos browser extension install

# Load in Chrome:
# 1. Navigate to chrome://extensions
# 2. Enable "Developer mode"
# 3. "Load unpacked" → select assets/chrome-extension/
```

Once installed, the extension auto-connects to the Cratos server via WebSocket on startup.

## Usage Examples

### 1. Site Search (search action)

```
[User] "Search for wireless keyboards on Naver Shopping"

[LLM autonomous decision → tool call]
browser(action: "search", site: "naver_shopping", query: "wireless keyboard")

[Result] Navigates to search page + auto-reads page text
→ LLM analyzes search results and summarizes for user
```

### 2. Text Click (click_text action)

```
[User] "Click the first product"

[LLM autonomous decision → tool call]
browser(action: "click_text", text: "wireless keyboard", index: 0)

[Result] Link detected → auto-navigation → auto-reads page text
→ LLM reviews product details
```

### 3. Login Automation

```
[User] "Log in to GitHub"

[LLM autonomous decision → multi-step tool calls]
1. browser(action: "navigate", url: "https://github.com/login")
2. browser(action: "fill", selector: "#login_field", value: "username")
3. browser(action: "fill", selector: "#password", value: "password")
4. browser(action: "click", selector: "input[type='submit']")
5. browser(action: "get_text")  ← verify result

[Result] GitHub login complete
```

### 4. Data Scraping

```
[User] "Get titles from Hacker News front page"

[LLM autonomous decision → tool calls]
1. browser(action: "navigate", url: "https://news.ycombinator.com")
2. browser(action: "get_text")

[Result] LLM extracts and organizes titles from page text
```

### 5. Screenshot

```
[User] "Take a screenshot of apple.com homepage"

[LLM autonomous decision → tool calls]
1. browser(action: "navigate", url: "https://apple.com")
2. browser(action: "screenshot", full_page: true)

[Result] [Screenshot image returned]
```

### 6. Tab Management

```
[User] "Show me the currently open tabs"

[LLM autonomous decision → tool call]
browser(action: "get_tabs")

[Result] List of open tabs (title, URL) returned
```

## Configuration

```toml
# config/default.toml
[browser]
enabled = true
default_engine = "playwright"  # playwright, puppeteer, chrome-devtools

# Playwright settings
[browser.playwright]
browser_type = "chromium"  # chromium, firefox, webkit
headless = true
slow_mo = 0  # Debug delay (ms)
timeout = 30000  # Default timeout (ms)

# Viewport
[browser.viewport]
width = 1280
height = 720

# Proxy (optional)
[browser.proxy]
server = ""
username = ""
password = ""

# User Agent (optional)
[browser.user_agent]
custom = ""
```

## Security Considerations

1. **Sandbox**: Browser runs in sandbox mode
2. **Domain Restriction**: Only allowed domains accessible (configurable)
3. **Credential Management**: Use environment variables or Vault for passwords
4. **Rate Limiting**: Automation speed limits
5. **Logging**: All browser actions logged (Replay integration)
6. **Text Truncation**: `get_text` auto-truncates at 8,000 chars, `get_html` at 15,000 chars (prevents token overflow)

```toml
# Security settings
[browser.security]
# Allowed domains (empty allows all)
allowed_domains = []
# Blocked domains
blocked_domains = ["localhost", "127.0.0.1"]
# Disable credential saving
save_credentials = false
```

## Replay Integration

Browser actions integrate with the Cratos Replay system:

```
[Browser Timeline]
┌────────────────────────────────────────────────────────────┐
│ 10:00:00 │ Search   │ naver_shopping: "wireless keyboard"  │
│ 10:00:02 │ ClickTxt │ "Logitech K380" (match 1/5)         │
│ 10:00:05 │ GetText  │ body (auto-read after navigation)    │
│ 10:00:06 │ Screenshot│ full_page                           │
├──────────┴──────────┴───────────────────────────────────────┤
│ [◀ Re-execute] [View Screenshot]                            │
└────────────────────────────────────────────────────────────┘
```

## How click_text Works

`click_text` operates in two phases:

1. **Phase 1**: JavaScript scans the page for text-matching elements
   - Prioritizes clickable elements: `a`, `button`, `[role="button"]`, `[onclick]`, etc.
   - Falls back to all elements if no clickable match found
   - Detects links (`<a href>`) and returns the URL; non-link elements are clicked directly

2. **Phase 2**: Follow-up based on result
   - **Link detected**: Navigate action triggers page load (with wait)
   - **Direct click**: 2-second wait, then URL change detection (handles JS-based navigation)
   - Page text is auto-read after any navigation

This allows interaction using visible text alone, without knowing CSS selectors.
