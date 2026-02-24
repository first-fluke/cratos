# Browser Automation - 자율 브라우저 제어

## 개요

Cratos는 LLM이 자율적으로 브라우저를 제어하는 방식으로 웹 자동화를 수행합니다. 사전 코딩된 워크플로우 없이, AI가 페이지를 읽고 다음 행동을 스스로 결정합니다.

### 브라우저 백엔드

| 백엔드 | 연결 방식 | 특징 |
|--------|-----------|------|
| **Chrome Extension** (기본) | WebSocket → Cratos 서버 | 사용자의 실제 브라우저 제어, 탭 목록 조회 가능 |
| **MCP (Playwright)** | JSON-RPC over stdio | 헤드리스 브라우저, 크로스 브라우저 지원 |
| **Auto** (권장) | Extension 우선, MCP 폴백 | 확장이 연결되면 확장 사용, 아니면 MCP 자동 전환 |

### 핵심 기능

| 기능 | 설명 |
|------|------|
| **자율 제어** | LLM이 페이지를 읽고 다음 행동을 자율 결정 (Plan-Act-Reflect) |
| **사이트 검색** | `search` 액션으로 네이버쇼핑, 쿠팡, 구글, 유튜브 등 자동 검색 URL 생성 |
| **텍스트 클릭** | `click_text`로 CSS 셀렉터 없이 화면에 보이는 텍스트로 클릭 |
| **탭 관리** | `get_tabs`로 열린 탭 목록 조회 (Chrome 확장 전용) |
| **페이지 분석** | `get_text`, `get_html`로 페이지 내용 읽기 (자동 truncation) |
| **스크린샷** | 전체 페이지 또는 특정 요소 캡처 |
| **폼 자동화** | `fill`, `type`, `select`, `check`으로 폼 입력 |
| **JS 실행** | `evaluate`로 임의 JavaScript 실행 |

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    Cratos Orchestrator                       │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                LLM (자율 에이전트)                       │ │
│  │    사용자 요청 분석 → 도구 선택 → 결과 해석 → 반복      │ │
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
│  사용자의 Chrome      │  │  Headless Browser    │
│  (확장 프로그램 설치) │  │  (Playwright MCP)    │
└──────────────────────┘  └──────────────────────┘
```

## 브라우저 액션 목록

Cratos의 `browser` 도구는 단일 도구에 `action` 파라미터로 다양한 액션을 제공합니다:

### 검색 & 탐색

| 액션 | 설명 | 필수 파라미터 |
|------|------|---------------|
| `search` | 지정 사이트에서 검색 (자동 URL 생성) | `site`, `query` |
| `navigate` | URL로 이동 | `url` |
| `go_back` | 뒤로 가기 | - |
| `go_forward` | 앞으로 가기 | - |
| `reload` | 새로고침 | - |
| `get_url` | 현재 URL 조회 | - |
| `get_title` | 페이지 제목 조회 | - |
| `get_tabs` | 열린 탭 목록 (Chrome 확장 전용) | - |

### 요소 조작

| 액션 | 설명 | 필수 파라미터 |
|------|------|---------------|
| `click` | CSS 셀렉터로 클릭 | `selector` |
| `click_text` | 화면에 보이는 텍스트로 클릭 | `text` |
| `type` | 텍스트 입력 (기존 값 유지) | `selector`, `text` |
| `fill` | 폼 필드 채우기 (기존 값 지우고 입력) | `selector`, `value` |
| `select` | 드롭다운 선택 | `selector`, `value` |
| `check` | 체크박스 체크/해제 | `selector` |
| `hover` | 마우스 오버 | `selector` |
| `press` | 키보드 키 입력 | `key` |
| `scroll` | 스크롤 | `x`, `y` |

### 정보 추출

| 액션 | 설명 | 필수 파라미터 |
|------|------|---------------|
| `get_text` | 텍스트 추출 (셀렉터 생략 시 전체 페이지) | - |
| `get_html` | HTML 추출 | - |
| `get_attribute` | 요소 속성 조회 | `selector`, `attribute` |
| `screenshot` | 스크린샷 캡처 | - |

### 대기 & 고급

| 액션 | 설명 | 필수 파라미터 |
|------|------|---------------|
| `wait_for_selector` | 요소 출현 대기 | `selector` |
| `wait_for_navigation` | 페이지 로드 대기 | - |
| `evaluate` | JavaScript 실행 | `script` |
| `close` | 브라우저 닫기 | - |

### 지원 검색 사이트

`search` 액션의 `site` 파라미터:

| 사이트 | 식별자 | 한글 식별자 |
|--------|--------|-------------|
| 네이버 쇼핑 | `naver_shopping` | `네이버쇼핑` |
| 네이버 | `naver` | `네이버` |
| 쿠팡 | `coupang` | `쿠팡` |
| 구글 | `google` | `구글` |
| 유튜브 | `youtube` | `유튜브` |
| 아마존 | `amazon` | `아마존` |
| 구글 맵 | `google_maps` | `구글맵` |

미지원 사이트는 자동으로 `site:` 구글 검색으로 폴백됩니다.

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
// .mcp.json (프로젝트 루트)
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

### 3. Chrome 확장 설정

```bash
# 확장 설치 경로 확인
cratos browser extension path

# 확장 설치
cratos browser extension install

# Chrome에서 확장 로드:
# 1. chrome://extensions 이동
# 2. "개발자 모드" 활성화
# 3. "압축해제된 확장 프로그램을 로드합니다" → assets/chrome-extension/ 선택
```

확장이 설치되면 Cratos 서버 시작 시 WebSocket으로 자동 연결됩니다.

## 사용 예시

### 1. 사이트 검색 (search 액션)

```
[사용자] "네이버 쇼핑에서 무선 키보드 검색해줘"

[LLM 자율 판단 → 도구 호출]
browser(action: "search", site: "naver_shopping", query: "무선 키보드")

[결과] 검색 페이지로 이동 + 페이지 텍스트 자동 읽기
→ LLM이 검색 결과를 분석하여 사용자에게 요약 전달
```

### 2. 텍스트 클릭 (click_text 액션)

```
[사용자] "첫 번째 상품 클릭해줘"

[LLM 자율 판단 → 도구 호출]
browser(action: "click_text", text: "무선 키보드", index: 0)

[결과] 링크 감지 → 자동 네비게이션 → 페이지 텍스트 자동 읽기
→ LLM이 상품 상세 정보 확인
```

### 3. 로그인 자동화

```
[사용자] "GitHub에 로그인해줘"

[LLM 자율 판단 → 다단계 도구 호출]
1. browser(action: "navigate", url: "https://github.com/login")
2. browser(action: "fill", selector: "#login_field", value: "username")
3. browser(action: "fill", selector: "#password", value: "password")
4. browser(action: "click", selector: "input[type='submit']")
5. browser(action: "get_text")  ← 결과 확인

[결과] GitHub 로그인 완료
```

### 4. 데이터 스크래핑

```
[사용자] "Hacker News 첫 페이지 제목들 가져와줘"

[LLM 자율 판단 → 도구 호출]
1. browser(action: "navigate", url: "https://news.ycombinator.com")
2. browser(action: "get_text")

[결과] LLM이 페이지 텍스트에서 제목 추출하여 정리
```

### 5. 스크린샷

```
[사용자] "apple.com 메인페이지 스크린샷 찍어줘"

[LLM 자율 판단 → 도구 호출]
1. browser(action: "navigate", url: "https://apple.com")
2. browser(action: "screenshot", full_page: true)

[결과] [스크린샷 이미지 반환]
```

### 6. 탭 관리

```
[사용자] "지금 열려있는 탭들 보여줘"

[LLM 자율 판단 → 도구 호출]
browser(action: "get_tabs")

[결과] 열린 탭 목록 (제목, URL) 반환
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
6. **텍스트 Truncation**: `get_text`는 8,000자, `get_html`은 15,000자로 자동 잘림 (토큰 오버플로우 방지)

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

```
[Browser Timeline]
┌────────────────────────────────────────────────────────────┐
│ 10:00:00 │ Search   │ naver_shopping: "무선 키보드"         │
│ 10:00:02 │ ClickTxt │ "로지텍 K380" (match 1/5)            │
│ 10:00:05 │ GetText  │ body (auto-read after navigation)    │
│ 10:00:06 │ Screenshot│ full_page                           │
├──────────┴──────────┴───────────────────────────────────────┤
│ [◀ 재실행] [스크린샷 보기]                                    │
└────────────────────────────────────────────────────────────┘
```

## click_text 동작 방식

`click_text`는 2단계로 동작합니다:

1. **Phase 1**: JavaScript로 화면에서 텍스트 매칭 요소 탐색
   - `a`, `button`, `[role="button"]`, `[onclick]` 등 클릭 가능 요소 우선
   - 매칭 실패 시 모든 요소로 범위 확대
   - 링크(`<a href>`) 감지 시 URL 반환, 비링크 요소는 직접 클릭

2. **Phase 2**: 결과에 따른 후속 처리
   - **링크 감지**: Navigate 액션으로 페이지 이동 (로드 대기)
   - **직접 클릭**: 2초 대기 후 URL 변경 감지 (JS 기반 네비게이션 처리)
   - 네비게이션 발생 시 페이지 텍스트 자동 읽기

이 방식으로 CSS 셀렉터를 모르더라도 화면에 보이는 텍스트만으로 조작할 수 있습니다.
