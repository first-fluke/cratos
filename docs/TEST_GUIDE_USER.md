# Cratos 테스트 가이드 - 비개발자용

## 테스트 목표

비개발자가 터미널 한 줄로 설치하고, 마법사를 따라 설정을 완료할 수 있는지 확인

---

## 사전 준비

1. **Telegram 계정** 필요 (봇 만들 때 사용)
2. **터미널/PowerShell** 열기
   - macOS: Spotlight에서 "터미널" 검색
   - Windows: 시작 메뉴에서 "PowerShell" 검색

---

## 테스트 시나리오

### 시나리오 1: 원클릭 설치 (릴리스 후)

> 참고: 아직 GitHub Release가 없으면 이 테스트는 건너뛰세요.

**macOS/Linux:**
```bash
curl -sSL https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.sh | sh
```

**Windows PowerShell:**
```powershell
irm https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.ps1 | iex
```

**확인 항목:**
- [ ] 다운로드가 자동으로 진행되는가?
- [ ] 설치 완료 메시지가 출력되는가?
- [ ] 마법사가 자동으로 시작되는가?

---

### 시나리오 2: Wizard 직접 실행 (개발 버전)

프로젝트 폴더에서:

```bash
# 한국어 마법사
cargo run -- init --lang ko

# 또는 영어 마법사
cargo run -- init --lang en

# 또는 시스템 언어 자동 감지
cargo run -- init
```

---

## 마법사 테스트 체크리스트

### Step 1: 환영 화면

- [ ] 환영 메시지가 보이는가?
- [ ] 3단계 설명이 표시되는가?
- [ ] 총 소요 시간(약 8분)이 표시되는가?

### Step 2: Telegram 봇 설정

- [ ] BotFather 링크(`https://t.me/BotFather`)가 표시되는가?
- [ ] 단계별 설명이 이해하기 쉬운가?
- [ ] "건너뛰기" 옵션이 있는가?

**테스트 A: 실제 봇 토큰 입력**
1. 링크 클릭 → Telegram 앱 열림
2. BotFather에게 `/newbot` 전송
3. 봇 이름, 사용자명 입력
4. 받은 토큰 복사 후 붙여넣기

**테스트 B: 건너뛰기**
1. "Telegram 설정 건너뛰기?" 에서 "Yes" 선택

- [ ] 토큰 입력이 마스킹 처리되는가? (******* 형태)
- [ ] 건너뛰기가 정상 동작하는가?

### Step 3: AI 모델 선택

- [ ] 무료/유료 옵션이 구분되어 표시되는가?
- [ ] 각 옵션에 가격 정보가 있는가?
- [ ] 화살표로 선택할 수 있는가?

**추천 테스트 순서:**
1. **Groq** (무료, 가장 쉬움) - https://console.groq.com/keys
2. **OpenRouter** (무료) - https://openrouter.ai/keys
3. **Google AI** (무료) - https://aistudio.google.com/apikey

### Step 4: API 키 입력

- [ ] 가입 링크가 표시되는가?
- [ ] 단계별 설명이 있는가?
- [ ] API 키가 마스킹 처리되는가?

### Step 5: 연결 테스트

- [ ] Telegram 연결 테스트가 실행되는가? (토큰 입력한 경우)
- [ ] LLM 연결 테스트가 실행되는가?
- [ ] 성공/실패 메시지가 명확한가?
- [ ] 실패 시 "계속할까요?" 옵션이 있는가?

### Step 6: 완료

- [ ] 완료 메시지가 표시되는가?
- [ ] 요약 정보가 표시되는가?
- [ ] 다음 단계 안내가 있는가?
- [ ] `.env` 파일이 생성되었는가?

---

## 생성된 .env 파일 확인

```bash
cat .env
```

**확인 항목:**
- [ ] 선택한 LLM 프로바이더 키가 저장되었는가?
- [ ] Telegram 토큰이 저장되었는가? (입력한 경우)
- [ ] `CRATOS_DEFAULT_PROVIDER` 값이 올바른가?

---

## 문제 발생 시

### 링크가 클릭 안 됨
- 터미널 앱 설정에서 "URL 클릭 활성화" 확인
- 또는 링크를 직접 복사해서 브라우저에 붙여넣기

### 한글이 깨짐
- 터미널 인코딩을 UTF-8로 설정

### API 키 입력이 안 됨
- 붙여넣기: Ctrl+V (Windows) 또는 Cmd+V (macOS)
- 입력 후 Enter 키

---

## 피드백 항목

테스트 후 아래 질문에 답해주세요:

1. 설명이 이해하기 쉬웠나요? (1-5점)
2. 링크를 찾기 쉬웠나요? (1-5점)
3. 막히는 부분이 있었나요? (있다면 어디?)
4. 개선 제안이 있나요?
