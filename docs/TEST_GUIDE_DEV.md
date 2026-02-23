# Cratos 테스트 가이드 - 개발자용

## 테스트 목표

1. init 명령어가 정상 동작하는지 확인
2. 다국어 지원이 제대로 되는지 확인
3. 설치 스크립트 검증
4. release.yml 워크플로우 검증

---

## 1. 기본 빌드 및 테스트

```bash
# 전체 테스트
cargo test --workspace

# 빌드 확인
cargo build --release

# CLI 도움말 확인
cargo run -- --help
cargo run -- init --help
```

**확인 항목:**
- [ ] 테스트 전체 통과
- [ ] 릴리스 빌드 성공
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

## 6. 통합 테스트

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

## 7. 엣지 케이스

### 7.1 빈 입력 처리

```bash
cargo run -- init --lang en
# API 키 입력에서 빈 값 입력 시 → 재입력 요청
```

### 7.2 Ctrl+C 처리

```bash
cargo run -- init --lang en
# 중간에 Ctrl+C → 깔끔하게 종료
```

### 7.3 잘못된 언어 코드

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
