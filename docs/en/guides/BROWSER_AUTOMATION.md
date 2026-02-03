# Browser Automation - MCP-Based Browser Control

## Overview

Automate browsers through Model Context Protocol (MCP) for web scraping, form filling, test automation, and more.

### Supported MCP Servers

| MCP Server | Browser Engine | Features |
|------------|----------------|----------|
| **playwright-mcp** | Chromium/Firefox/WebKit | Cross-browser, stable |
| **puppeteer-mcp** | Chromium | Fast, Google-backed |
| **chrome-devtools** | Chrome | Direct DevTools Protocol |

### Core Features

| Feature | Description |
|---------|-------------|
| **Page Navigation** | URL navigation, back/forward, refresh |
| **Element Interaction** | Click, type, scroll, drag |
| **Screenshots** | Full page, element capture |
| **DOM Analysis** | Find elements, extract text |
| **Network Monitoring** | Intercept requests/responses |
| **Form Automation** | Login, search, data entry |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Cratos Orchestrator                       │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                    Tool Registry                         │ │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐           │ │
│  │  │ MCP Client│  │ MCP Client│  │ MCP Client│           │ │
│  │  │ (Browser) │  │ (Files)   │  │ (Search)  │           │ │
│  │  └─────┬─────┘  └───────────┘  └───────────┘           │ │
│  └────────│────────────────────────────────────────────────┘ │
└───────────│─────────────────────────────────────────────────┘
            │ MCP Protocol (JSON-RPC over stdio/SSE)
            ▼
┌─────────────────────────────────────────────────────────────┐
│                    MCP Server (Browser)                      │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                  Playwright/Puppeteer                    │ │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐           │ │
│  │  │  Browser  │  │  Page     │  │  Element  │           │ │
│  │  │  Manager  │  │  Manager  │  │  Selector │           │ │
│  │  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘           │ │
│  └────────│──────────────│──────────────│──────────────────┘ │
└───────────│──────────────│──────────────│───────────────────┘
            ▼              ▼              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Browser Engine                            │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐               │
│  │ Chromium  │  │  Firefox  │  │  WebKit   │               │
│  └───────────┘  └───────────┘  └───────────┘               │
└─────────────────────────────────────────────────────────────┘
```

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
// ~/.cratos/mcp.json
{
  "mcpServers": {
    "playwright": {
      "command": "npx",
      "args": ["@anthropic-ai/mcp-server-playwright"],
      "env": {
        "BROWSER_TYPE": "chromium",
        "HEADLESS": "true"
      }
    },
    "chrome-devtools": {
      "command": "npx",
      "args": ["@anthropic-ai/mcp-server-chrome-devtools"],
      "env": {
        "CHROME_PATH": "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
      }
    }
  }
}
```

## Usage Examples

### 1. Web Search

```
[User] "Search for 'Rust async' on Google and tell me the first result"

[AI Tool Calls]
1. browser.navigate(url: "https://google.com")
2. browser.type(selector: "input[name='q']", text: "Rust async")
3. browser.click(selector: "input[name='btnK']")
4. browser.wait(selector: "#search")
5. browser.get_text(selector: "#search .g:first-child h3")

[Result] First search result: "Asynchronous Programming in Rust - Rust Book"
```

### 2. Login Automation

```
[User] "Log in to GitHub"

[AI Tool Calls]
1. browser.navigate(url: "https://github.com/login")
2. browser.type(selector: "#login_field", text: "${GITHUB_USERNAME}")
3. browser.type(selector: "#password", text: "${GITHUB_PASSWORD}")
4. browser.click(selector: "input[type='submit']")
5. browser.wait(selector: ".avatar")

[Result] GitHub login complete
```

### 3. Data Scraping

```
[User] "Get titles from Hacker News front page"

[AI Tool Calls]
1. browser.navigate(url: "https://news.ycombinator.com")
2. browser.get_text(selector: ".titleline > a")

[Result]
1. Show HN: I built a self-hosted AI assistant
2. Rust 2024 Survey Results
3. The History of Web Browsers
...
```

### 4. Screenshot

```
[User] "Take a screenshot of apple.com homepage"

[AI Tool Calls]
1. browser.navigate(url: "https://apple.com")
2. browser.screenshot(full_page: true)

[Result] [Screenshot image returned]
```

## MCP Tool Schemas

### navigate

```json
{
  "name": "navigate",
  "description": "Navigate to a URL",
  "inputSchema": {
    "type": "object",
    "properties": {
      "url": {
        "type": "string",
        "description": "URL to navigate to"
      },
      "waitUntil": {
        "type": "string",
        "enum": ["load", "domcontentloaded", "networkidle"],
        "default": "load"
      }
    },
    "required": ["url"]
  }
}
```

### click

```json
{
  "name": "click",
  "description": "Click an element",
  "inputSchema": {
    "type": "object",
    "properties": {
      "selector": {
        "type": "string",
        "description": "CSS selector for element to click"
      },
      "button": {
        "type": "string",
        "enum": ["left", "right", "middle"],
        "default": "left"
      },
      "clickCount": {
        "type": "integer",
        "default": 1
      }
    },
    "required": ["selector"]
  }
}
```

### type

```json
{
  "name": "type",
  "description": "Type text into an input field",
  "inputSchema": {
    "type": "object",
    "properties": {
      "selector": {
        "type": "string",
        "description": "CSS selector for input element"
      },
      "text": {
        "type": "string",
        "description": "Text to type"
      },
      "delay": {
        "type": "integer",
        "description": "Delay between keystrokes in ms",
        "default": 0
      },
      "clear": {
        "type": "boolean",
        "description": "Clear existing text first",
        "default": false
      }
    },
    "required": ["selector", "text"]
  }
}
```

### screenshot

```json
{
  "name": "screenshot",
  "description": "Take a screenshot",
  "inputSchema": {
    "type": "object",
    "properties": {
      "selector": {
        "type": "string",
        "description": "CSS selector for element to screenshot (optional)"
      },
      "fullPage": {
        "type": "boolean",
        "description": "Capture full scrollable page",
        "default": false
      },
      "format": {
        "type": "string",
        "enum": ["png", "jpeg"],
        "default": "png"
      },
      "quality": {
        "type": "integer",
        "description": "JPEG quality (0-100)",
        "default": 80
      }
    }
  }
}
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

Browser actions integrate with Cratos Replay system:

```rust
/// Browser event (for Replay storage)
#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserEvent {
    pub timestamp: DateTime<Utc>,
    pub action: BrowserAction,
    pub selector: Option<String>,
    pub url: Option<String>,
    pub screenshot: Option<String>,  // Base64
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BrowserAction {
    Navigate { url: String },
    Click { selector: String },
    Type { selector: String, text: String },
    Screenshot { path: String },
    GetText { selector: String, result: String },
}
```

### Timeline View

```
[Browser Timeline]
┌────────────────────────────────────────────────────────────┐
│ 10:00:00 │ Navigate │ https://google.com                   │
│ 10:00:02 │ Type     │ input[name='q'] <- "Rust async"     │
│ 10:00:03 │ Click    │ input[name='btnK']                  │
│ 10:00:05 │ Wait     │ #search (found in 1.2s)             │
│ 10:00:05 │ GetText  │ .g:first-child h3                   │
├──────────┴──────────┴───────────────────────────────────────┤
│ [◀ Re-execute] [View Screenshot 📷]                         │
└────────────────────────────────────────────────────────────┘
```

## Roadmap

1. **v1.0**: Basic browser automation (navigate, click, type, screenshot)
2. **v1.1**: Network interception, PDF generation
3. **v1.2**: Multi-page, tab management
4. **v2.0**: Visual element recognition (AI-based selectors)
