# Cratos 테스트 가이드 - 개발자용

## 테스트 목표

1. 자동화 테스트 스위트 전체 통과 확인 (1,286+ 테스트)
2. init 명령어가 정상 동작하는지 확인
3. 다국어 지원이 제대로 되는지 확인
4. 설치 스크립트 검증
5. release.yml CI 워크플로우 검증

---

## 1. 기본 빌드 및 테스트

```bash
# 전체 테스트 (1,286+ tests)
cargo test --workspace

# 빠른 타입 체크
cargo check --all-targets

# 린트
cargo clippy --all-targets

# 빌드 확인 (일상 개발용)
cargo build --profile dev-release -p cratos

# 릴리스 빌드 (배포용, ~10분)
cargo build --release

# CLI 도움말 확인
cargo run -- --help
cargo run -- init --help
```

**확인 항목:**
- [ ] 테스트 전체 통과 (1,286+ tests, 0 failures)
- [ ] clippy 경고 없음
- [ ] 빌드 성공
- [ ] init 명령어가 help에 표시됨
- [ ] `--lang` 옵션이 표시됨

---

## 2. Wizard 기능 테스트

### 2.1 언어 감지 테스트

```bash
# 영어 강제
cargo run -- init --lang en

# 한국어 강제
cargo run -- init --lang ko

# 시스템 언어 감지 (LANG 환경변수 기반)
LANG=ko_KR.UTF-8 cargo run -- init
LANG=en_US.UTF-8 cargo run -- init
```

**확인 항목:**
- [ ] `--lang en` → 영어 출력
- [ ] `--lang ko` → 한국어 출력
- [ ] `LANG=ko_KR` → 한국어 자동 감지
- [ ] `LANG=en_US` → 영어 자동 감지

### 2.2 기존 .env 파일 처리

```bash
# .env 파일이 있는 상태에서 실행
echo "TEST=1" > .env
cargo run -- init --lang en
# "Overwrite?" 프롬프트가 나와야 함
```

**확인 항목:**
- [ ] 기존 .env 존재 시 덮어쓰기 확인 프롬프트
- [ ] "No" 선택 시 취소 메시지
- [ ] "Yes" 선택 시 진행

### 2.3 Telegram 건너뛰기

```bash
cargo run -- init --lang en
# Step 1에서 "Skip Telegram setup?" → Yes
```

**확인 항목:**
- [ ] 건너뛰기 후 Step 2로 진행
- [ ] 최종 .env에 `# TELEGRAM_BOT_TOKEN=` 주석 처리

### 2.4 프로바이더 선택

각 프로바이더 선택 후 확인:

| 프로바이더 | 예상 env_var | 확인 |
|-----------|-------------|------|
| OpenRouter | `OPENROUTER_API_KEY` | [ ] |
| Groq | `GROQ_API_KEY` | [ ] |
| Google AI | `GEMINI_API_KEY` (or `GOOGLE_API_KEY`) | [ ] |
| OpenAI | `OPENAI_API_KEY` | [ ] |
| Anthropic | `ANTHROPIC_API_KEY` | [ ] |
| DeepSeek | `DEEPSEEK_API_KEY` | [ ] |
| Ollama | `OLLAMA_BASE_URL` | [ ] |

### 2.5 연결 테스트 로직

```bash
# Ollama 테스트 (Ollama 실행 중일 때)
ollama serve &
cargo run -- init --lang en
# Ollama 선택 → 연결 성공해야 함

# Ollama 테스트 (Ollama 미실행)
pkill ollama
cargo run -- init --lang en
# Ollama 선택 → 연결 실패 + "Continue anyway?" 프롬프트
```

**확인 항목:**
- [ ] Ollama 실행 중 → 연결 성공
- [ ] Ollama 미실행 → 연결 실패 메시지 + 계속 진행 옵션

### 2.6 Telegram 토큰 검증

```bash
cargo run -- init --lang en
# 유효한 토큰 입력 → 성공
# 잘못된 토큰 입력 → 실패 + "Continue anyway?"
```

**확인 항목:**
- [ ] 유효한 토큰 → "Success!"
- [ ] 잘못된 토큰 → "Failed" + 계속 옵션

---

## 3. 설치 스크립트 테스트

### 3.1 install.sh 문법 검증

```bash
# shellcheck으로 문법 검사
shellcheck scripts/install.sh

# 실행 권한 확인
ls -la scripts/install.sh
# -rwxr-xr-x 이어야 함
```

### 3.2 install.sh 드라이런

```bash
# 스크립트 내용 확인 (실행 없이)
cat scripts/install.sh

# 플랫폼 감지 함수 테스트
bash -c 'source scripts/install.sh; detect_os'
bash -c 'source scripts/install.sh; detect_arch'
bash -c 'source scripts/install.sh; get_target'
```

### 3.3 install.ps1 문법 검증 (Windows 또는 PowerShell Core)

```powershell
# PowerShell에서 문법 검사
$script = Get-Content scripts/install.ps1 -Raw
[System.Management.Automation.PSParser]::Tokenize($script, [ref]$null)
```

---

## 4. Release Workflow 검증

### 4.1 YAML 문법 검사

```bash
# yamllint 설치 (brew install yamllint)
yamllint .github/workflows/release.yml
```

### 4.2 빌드 매트릭스 확인

`release.yml`에서 다음 타겟 확인:

| Target | OS Runner | 확인 |
|--------|-----------|------|
| `x86_64-apple-darwin` | `macos-13` | [ ] |
| `aarch64-apple-darwin` | `macos-14` | [ ] |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | [ ] |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` + cross | [ ] |
| `x86_64-pc-windows-msvc` | `windows-latest` | [ ] |

### 4.3 로컬 크로스 빌드 테스트 (선택)

```bash
# 현재 플랫폼 빌드
cargo build --release

# 다른 타겟 빌드 (rustup target add 필요)
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

---

## 5. 생성된 .env 파일 검증

```bash
# init 완료 후
cat .env

# 예상 구조:
# - LLM Provider 섹션
# - CRATOS_LLM__DEFAULT_PROVIDER 설정
# - Telegram 섹션
# - Server 섹션 (HOST, PORT)
# - Logging 섹션 (RUST_LOG)
# - Default Persona 섹션
```

**확인 항목:**
- [ ] 주석이 적절히 포함됨
- [ ] 섹션 구분이 명확함
- [ ] 민감 정보가 올바르게 저장됨

---

## 6. 자동화 테스트 구조

### 6.1 테스트 카테고리 개요

| 카테고리 | 위치 | 설명 |
|----------|------|------|
| **도구 레지스트리** | `crates/cratos-tools/src/builtins/mod.rs` | 23개 빌트인 도구 등록 및 카운트 검증 |
| **개별 도구** | `crates/cratos-tools/src/builtins/*.rs` | 도구별 정의, 입력 검증, 보안 테스트 |
| **오케스트레이터** | `crates/cratos-core/src/orchestrator/tests.rs` | 설정, 입력, 에러 새니타이즈, tool refusal 휴리스틱 |
| **새니타이즈** | `crates/cratos-core/src/orchestrator/sanitize.rs` | `is_tool_refusal`, `is_fake_tool_use_text`, 에러 새니타이즈 |
| **통합 테스트** | `tests/integration_test.rs` | 크레이트 간 통합 (LLM, 도구, 리플레이, 채널) |
| **LLM 프로바이더** | `crates/cratos-llm/src/` | 모델 티어, 라우팅 규칙, 프로바이더 설정 |
| **리플레이** | `crates/cratos-replay/src/` | 이벤트 스토어, 실행 생명주기 |
| **스킬** | `crates/cratos-skills/src/` | 스킬 생성, 라우팅, 레지스트리 |
| **메모리** | `crates/cratos-memory/src/` | Graph RAG, 대화 메모리 |
| **보안** | `crates/cratos-core/src/security/` | 레이트 리미터, 서킷 브레이커 |

### 6.2 빌트인 도구 목록 (23개)

통합 테스트(`tests/integration_test.rs`)에서 검증하는 전체 도구 목록:

```
file_read, file_write, file_list, http_get, http_post, exec, bash,
git_status, git_commit, git_branch, git_diff, git_push, git_clone, git_log,
github_api, browser, wol, config, web_search, agent_cli,
send_file, image_generate, app_control
```

> **주의**: 도구 추가/제거 시 3곳 동기화 필수:
> 1. `crates/cratos-tools/src/builtins/mod.rs` — 등록 + 테스트 카운트
> 2. `tests/integration_test.rs` — `expected_tools` 배열 및 카운트
> 3. 도구별 테스트 파일

### 6.3 특정 크레이트/모듈 테스트

```bash
# 도구 레지스트리 테스트만
cargo test -p cratos-tools

# 오케스트레이터 테스트만
cargo test -p cratos-core

# 통합 테스트만
cargo test --test integration_test

# 특정 테스트 함수
cargo test test_tool_registry_with_builtins
cargo test test_tool_refusal
cargo test test_fake_tool_use_detection
```

### 6.4 오케스트레이터 (ReAct 루프) 테스트

Workflow Engine은 제거되었으며, 자율 ReAct 루프로 대체되었습니다. 관련 테스트:

| 테스트 | 검증 내용 |
|--------|----------|
| `test_tool_refusal_*` | LLM이 도구 호출 없이 짧은 텍스트만 반환하는 경우 감지 |
| `test_fake_tool_use_detection` | `[Used 1 tool: browser:OK]` 같은 가짜 도구 사용 텍스트 감지 |
| `test_sanitize_error_for_user` | 경로 등 민감 정보 마스킹 |
| `test_sanitize_for_session_memory` | 프롬프트 인젝션 방지 |
| `test_orchestrator_config_failure_limits` | 연속/총 실패 횟수 제한 설정 |
| `test_max_execution_secs_default` | 실행 타임아웃 기본값 (180초) |

> **참고**: `is_tool_refusal` 함수는 `sanitize.rs`에 있지만 테스트는 `orchestrator/tests.rs`에 있습니다.

### 6.5 app_control 도구 테스트

`app_control`은 macOS AppleScript/JXA 자동화 도구로, `RiskLevel::High`로 분류됩니다.

```bash
# app_control 테스트
cargo test -p cratos-tools app_control
```

테스트 항목:
- 도구 정의 (이름, 설명, 파라미터 스키마)
- 보안 검증 (`BLOCKED_PATTERNS`: `do shell script`, `System Preferences`, `password` 등 차단)

### 6.6 통합 테스트 상세

`tests/integration_test.rs`에서 크레이트 간 통합을 검증합니다:

- **LLM 라우터**: 프로바이더 설정, 라우팅 규칙, 모델 티어별 기본 모델
- **도구 레지스트리**: 23개 빌트인 도구 등록 확인, 스키마 검증
- **리플레이**: 실행 생명주기, 이벤트 타입, 상태 전이
- **오케스트레이터**: 입력 생성, 세션 키, 설정
- **채널**: 메시지 정규화 (Telegram, Slack)
- **보안**: 레이트 리미터, 서킷 브레이커, 메트릭

---

## 7. E2E 통합 테스트

```bash
# 1. 깨끗한 상태에서 시작
rm -f .env

# 2. init로 설정
cargo run -- init --lang ko

# 3. doctor로 검증
cargo run -- doctor

# 4. serve 시작 (Ctrl+C로 종료)
cargo run -- serve
```

**확인 항목:**
- [ ] init → .env 생성
- [ ] doctor → 설정 검증 통과
- [ ] serve → 서버 시작 성공

---

## 8. 엣지 케이스

### 8.1 빈 입력 처리

```bash
cargo run -- init --lang en
# API 키 입력에서 빈 값 입력 시 → 재입력 요청
```

### 8.2 Ctrl+C 처리

```bash
cargo run -- init --lang en
# 중간에 Ctrl+C → 깔끔하게 종료
```

### 8.3 잘못된 언어 코드

```bash
cargo run -- init --lang fr
# → 영어로 폴백 (기본값)
```

---

## 버그 리포트 템플릿

```
## 환경
- OS:
- Rust 버전:
- 명령어:

## 예상 동작

## 실제 동작

## 재현 단계
1.
2.
3.

## 로그/스크린샷
```
