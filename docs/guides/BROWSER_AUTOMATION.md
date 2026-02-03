# Browser Automation - MCP 기반 브라우저 자동화

## 개요

Model Context Protocol (MCP)을 통해 브라우저를 자동화하여 웹 스크래핑, 폼 입력, 테스트 자동화 등을 수행합니다.

### 지원 MCP 서버

| MCP 서버 | 브라우저 엔진 | 특징 |
|----------|---------------|------|
| **playwright-mcp** | Chromium/Firefox/WebKit | 크로스 브라우저, 안정적 |
| **puppeteer-mcp** | Chromium | 빠름, Google 지원 |
| **chrome-devtools** | Chrome | DevTools Protocol 직접 |

### 핵심 기능

| 기능 | 설명 |
|------|------|
| **페이지 탐색** | URL 이동, 뒤로/앞으로, 새로고침 |
| **요소 조작** | 클릭, 입력, 스크롤, 드래그 |
| **스크린샷** | 전체 페이지, 요소별 캡처 |
| **DOM 분석** | 요소 찾기, 텍스트 추출 |
| **네트워크 감시** | 요청/응답 가로채기 |
| **폼 자동화** | 로그인, 검색, 데이터 입력 |

## 아키텍처

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

## MCP 서버 설정

### 1. Playwright MCP (권장)

```bash
# 설치
npm install -g @anthropic-ai/mcp-server-playwright

# 또는 npx로 직접 실행
npx @anthropic-ai/mcp-server-playwright
```

### 2. MCP 설정 파일

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

## Cratos MCP 통합

### MCP 클라이언트 (Rust)

```rust
// crates/cratos-tools/src/mcp/client.rs

use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// MCP 서버 클라이언트
pub struct McpClient {
    process: Child,
    request_id: u64,
}

impl McpClient {
    /// MCP 서버 시작
    pub fn new(command: &str, args: &[&str]) -> Result<Self> {
        let process = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Self { process, request_id: 0 })
    }

    /// 도구 목록 조회
    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let request = McpRequest {
            jsonrpc: "2.0",
            id: self.next_id(),
            method: "tools/list",
            params: None,
        };

        let response: McpResponse<ToolsListResult> = self.send(request).await?;
        Ok(response.result.tools)
    }

    /// 도구 실행
    pub async fn call_tool(&mut self, name: &str, args: serde_json::Value) -> Result<serde_json::Value> {
        let request = McpRequest {
            jsonrpc: "2.0",
            id: self.next_id(),
            method: "tools/call",
            params: Some(json!({
                "name": name,
                "arguments": args
            })),
        };

        let response: McpResponse<ToolCallResult> = self.send(request).await?;
        Ok(response.result.content)
    }
}

/// MCP 도구 정의
#[derive(Debug, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

### 브라우저 도구 래퍼

```rust
// crates/cratos-tools/src/browser.rs

use crate::mcp::McpClient;
use crate::Tool;

/// 브라우저 자동화 도구
pub struct BrowserTool {
    mcp_client: McpClient,
}

impl BrowserTool {
    pub async fn new() -> Result<Self> {
        let mcp_client = McpClient::new("npx", &["@anthropic-ai/mcp-server-playwright"])?;
        Ok(Self { mcp_client })
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "웹 브라우저를 제어하여 페이지 탐색, 요소 조작, 데이터 추출 등을 수행합니다."
    }

    async fn execute(&mut self, input: ToolInput) -> Result<ToolOutput> {
        let action = input.get_string("action")?;

        match action.as_str() {
            "navigate" => self.navigate(&input).await,
            "click" => self.click(&input).await,
            "type" => self.type_text(&input).await,
            "screenshot" => self.screenshot(&input).await,
            "get_text" => self.get_text(&input).await,
            "wait" => self.wait(&input).await,
            _ => Err(Error::UnknownAction(action)),
        }
    }
}

impl BrowserTool {
    /// 페이지 이동
    async fn navigate(&mut self, input: &ToolInput) -> Result<ToolOutput> {
        let url = input.get_string("url")?;

        self.mcp_client.call_tool("navigate", json!({
            "url": url
        })).await?;

        Ok(ToolOutput::success(format!("Navigated to: {}", url)))
    }

    /// 요소 클릭
    async fn click(&mut self, input: &ToolInput) -> Result<ToolOutput> {
        let selector = input.get_string("selector")?;

        self.mcp_client.call_tool("click", json!({
            "selector": selector
        })).await?;

        Ok(ToolOutput::success(format!("Clicked: {}", selector)))
    }

    /// 텍스트 입력
    async fn type_text(&mut self, input: &ToolInput) -> Result<ToolOutput> {
        let selector = input.get_string("selector")?;
        let text = input.get_string("text")?;

        self.mcp_client.call_tool("type", json!({
            "selector": selector,
            "text": text
        })).await?;

        Ok(ToolOutput::success(format!("Typed '{}' into {}", text, selector)))
    }

    /// 스크린샷
    async fn screenshot(&mut self, input: &ToolInput) -> Result<ToolOutput> {
        let selector = input.get_string_opt("selector");
        let full_page = input.get_bool_opt("full_page").unwrap_or(false);

        let result = self.mcp_client.call_tool("screenshot", json!({
            "selector": selector,
            "fullPage": full_page
        })).await?;

        // Base64 이미지 반환
        Ok(ToolOutput::image(result["data"].as_str().unwrap()))
    }

    /// 텍스트 추출
    async fn get_text(&mut self, input: &ToolInput) -> Result<ToolOutput> {
        let selector = input.get_string("selector")?;

        let result = self.mcp_client.call_tool("get_text", json!({
            "selector": selector
        })).await?;

        Ok(ToolOutput::text(result["text"].as_str().unwrap()))
    }

    /// 대기
    async fn wait(&mut self, input: &ToolInput) -> Result<ToolOutput> {
        let selector = input.get_string_opt("selector");
        let timeout = input.get_u64_opt("timeout").unwrap_or(5000);

        if let Some(sel) = selector {
            self.mcp_client.call_tool("wait_for_selector", json!({
                "selector": sel,
                "timeout": timeout
            })).await?;
            Ok(ToolOutput::success(format!("Element found: {}", sel)))
        } else {
            tokio::time::sleep(tokio::time::Duration::from_millis(timeout)).await;
            Ok(ToolOutput::success(format!("Waited {} ms", timeout)))
        }
    }
}
```

## 사용 예시

### 1. 웹 검색

```
[사용자] "구글에서 'Rust async' 검색해서 첫번째 결과 알려줘"

[AI 도구 호출]
1. browser.navigate(url: "https://google.com")
2. browser.type(selector: "input[name='q']", text: "Rust async")
3. browser.click(selector: "input[name='btnK']")
4. browser.wait(selector: "#search")
5. browser.get_text(selector: "#search .g:first-child h3")

[결과] 첫번째 검색 결과: "Asynchronous Programming in Rust - Rust Book"
```

### 2. 로그인 자동화

```
[사용자] "GitHub에 로그인해줘"

[AI 도구 호출]
1. browser.navigate(url: "https://github.com/login")
2. browser.type(selector: "#login_field", text: "${GITHUB_USERNAME}")
3. browser.type(selector: "#password", text: "${GITHUB_PASSWORD}")
4. browser.click(selector: "input[type='submit']")
5. browser.wait(selector: ".avatar")

[결과] GitHub 로그인 완료
```

### 3. 데이터 스크래핑

```
[사용자] "Hacker News 첫 페이지 제목들 가져와줘"

[AI 도구 호출]
1. browser.navigate(url: "https://news.ycombinator.com")
2. browser.get_text(selector: ".titleline > a")

[결과]
1. Show HN: I built a self-hosted AI assistant
2. Rust 2024 Survey Results
3. The History of Web Browsers
...
```

### 4. 스크린샷

```
[사용자] "apple.com 메인페이지 스크린샷 찍어줘"

[AI 도구 호출]
1. browser.navigate(url: "https://apple.com")
2. browser.screenshot(full_page: true)

[결과] [스크린샷 이미지 반환]
```

## 고급 기능

### 네트워크 가로채기

```rust
/// 네트워크 요청 가로채기
async fn intercept_requests(&mut self, pattern: &str) -> Result<Vec<NetworkRequest>> {
    self.mcp_client.call_tool("network_intercept", json!({
        "urlPattern": pattern,
        "action": "log"
    })).await
}

/// API 응답 모킹
async fn mock_response(&mut self, url: &str, response: &str) -> Result<()> {
    self.mcp_client.call_tool("network_mock", json!({
        "url": url,
        "response": {
            "status": 200,
            "body": response
        }
    })).await
}
```

### 다중 페이지

```rust
/// 새 페이지 열기
async fn new_page(&mut self) -> Result<String> {
    let result = self.mcp_client.call_tool("new_page", json!({})).await?;
    Ok(result["pageId"].as_str().unwrap().to_string())
}

/// 페이지 전환
async fn switch_page(&mut self, page_id: &str) -> Result<()> {
    self.mcp_client.call_tool("switch_page", json!({
        "pageId": page_id
    })).await?;
    Ok(())
}
```

### PDF 생성

```rust
/// 페이지를 PDF로 저장
async fn to_pdf(&mut self, path: &str) -> Result<()> {
    self.mcp_client.call_tool("pdf", json!({
        "path": path,
        "format": "A4",
        "printBackground": true
    })).await?;
    Ok(())
}
```

## MCP 도구 스키마

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

## 설정

```toml
# config/default.toml
[browser]
enabled = true
default_engine = "playwright"  # playwright, puppeteer, chrome-devtools

# Playwright 설정
[browser.playwright]
browser_type = "chromium"  # chromium, firefox, webkit
headless = true
slow_mo = 0  # 디버깅용 지연 (ms)
timeout = 30000  # 기본 타임아웃 (ms)

# 뷰포트
[browser.viewport]
width = 1280
height = 720

# 프록시 (선택적)
[browser.proxy]
server = ""
username = ""
password = ""

# 사용자 에이전트 (선택적)
[browser.user_agent]
custom = ""
```

## 보안 고려사항

1. **샌드박스**: 브라우저는 샌드박스 모드로 실행
2. **도메인 제한**: 허용된 도메인만 접근 가능 (설정 가능)
3. **자격 증명 관리**: 비밀번호는 환경 변수 또는 Vault 사용
4. **Rate Limiting**: 자동화 속도 제한
5. **로깅**: 모든 브라우저 작업 로깅 (Replay 연동)

```toml
# 보안 설정
[browser.security]
# 허용 도메인 (비어있으면 모두 허용)
allowed_domains = []
# 차단 도메인
blocked_domains = ["localhost", "127.0.0.1"]
# 자격 증명 저장 비활성화
save_credentials = false
```

## Replay 연동

브라우저 작업은 Cratos Replay 시스템과 통합됩니다:

```rust
/// 브라우저 이벤트 (Replay 저장용)
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

### 타임라인 보기

```
[Browser Timeline]
┌────────────────────────────────────────────────────────────┐
│ 10:00:00 │ Navigate │ https://google.com                   │
│ 10:00:02 │ Type     │ input[name='q'] <- "Rust async"     │
│ 10:00:03 │ Click    │ input[name='btnK']                  │
│ 10:00:05 │ Wait     │ #search (found in 1.2s)             │
│ 10:00:05 │ GetText  │ .g:first-child h3                   │
├──────────┴──────────┴───────────────────────────────────────┤
│ [◀ 재실행] [스크린샷 보기 📷]                                 │
└────────────────────────────────────────────────────────────┘
```

## 향후 계획

1. **v1.0**: 기본 브라우저 자동화 (navigate, click, type, screenshot)
2. **v1.1**: 네트워크 가로채기, PDF 생성
3. **v1.2**: 다중 페이지, 탭 관리
4. **v2.0**: 시각적 요소 인식 (AI 기반 셀렉터)
