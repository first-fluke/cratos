# Cratos - AI-Powered Personal Assistant

> **Product Requirements Document (PRD)**
> Version: 1.0 | Last Updated: 2025-01

---

## 1. 목적 (Goals)

### 1.1 제품 목적

사용자는 **Telegram / Slack / Discord** 중 어디에서든 자연어로 말하면, 에이전트가:

1. **이해하고** (의도 파악)
2. **필요한 정보를 모으고** (문서/코드/로그/웹)
3. **실제로 실행하고** (툴/코드/명령)
4. **결과를 정리해** (요약/근거/다음 행동) 다시 같은 채널로 보고한다.

| 사용자 유형 | 주요 가치 |
|-------------|-----------|
| 비개발자 | 읽을 것 줄이기, 업무 준비 자동화, 반복 작업 처리, 통제감 |
| 개발자 | 원격 개발지시 (코딩→테스트→PR) end-to-end 자동화 |

### 1.2 핵심 차별화 (vs 기존 솔루션)

| 기능 | 기존 솔루션 | Cratos |
|------|-------------|--------|
| **리플레이 (되감기)** | X | 타임라인 + 재실행 + dry-run |
| **모델 라우팅** | X | 작업별 자동 선택, 70% 비용 절감 목표 |
| **원격 개발 E2E** | 부분적 | Issue → PR 완전 자동화 |
| **Tool Doctor** | X | 자기 진단 + 해결 체크리스트 |

### 1.3 해결하려는 문제 (사용자 관점)

- "알림/스레드/문서가 너무 많아서 뭘 해야 할지 모르겠다"
- "회의/업무 준비를 매번 처음부터 정리하느라 시간이 든다"
- "반복 작업(정리/요약/리포트/파일관리)이 계속 쌓인다"
- "원격으로 '이 이슈 고쳐서 PR 올려줘' 같은 지시를 안전하고 재현 가능하게 하고 싶다"
- "AI가 뭘 했는지 모르겠어서 불안하다 → 기록/되감기가 필요하다"

### 1.4 성공 기준

| 지표 | 목표 |
|------|------|
| 비개발자 온보딩 | 스레드 요약+할 일 추출, 회의 준비 브리핑을 설치 후 10분 내 수행 |
| 개발자 E2E | "이 이슈 고쳐서 PR" → 브랜치 생성→수정→테스트→PR 생성→요약 보고 1회 이상 성공 |
| 응답 속도 | 1차 응답(진행/질문/결과) 10초 내 95% 이상 |
| 감사 로그 | 모든 실행에 리플레이 기록 남음, 누락률 0% |

---

## 2. 시스템 아키텍처

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        User Channels                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                      │
│  │ Telegram │  │  Slack   │  │ Discord  │                      │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                      │
│       │             │             │                             │
│       └─────────────┼─────────────┘                             │
│                     ▼                                           │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  Channel Adapter Layer                   │   │
│  │  (cratos-channels: 메시지 정규화, 응답 라우팅)           │   │
│  └─────────────────────────┬───────────────────────────────┘   │
└────────────────────────────┼────────────────────────────────────┘
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Core Orchestration                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    cratos-core                           │   │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────────────┐    │   │
│  │  │  Planner  │  │  Executor │  │  Memory Manager   │    │   │
│  │  │ (계획수립) │  │ (실행루프) │  │ (컨텍스트 관리)   │    │   │
│  │  └───────────┘  └───────────┘  └───────────────────┘    │   │
│  └─────────────────────────┬───────────────────────────────┘   │
└────────────────────────────┼────────────────────────────────────┘
                             ▼
┌────────────────────────────┼────────────────────────────────────┐
│           ┌────────────────┼────────────────┐                   │
│           ▼                ▼                ▼                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐         │
│  │ cratos-llm  │  │cratos-tools │  │ cratos-replay   │         │
│  │ (모델라우팅) │  │ (도구실행)   │  │ (이벤트 기록)   │         │
│  └──────┬──────┘  └──────┬──────┘  └────────┬────────┘         │
│         │                │                  │                   │
│         ▼                ▼                  ▼                   │
│  ┌───────────┐    ┌───────────┐     ┌───────────────┐          │
│  │  OpenAI   │    │  Files    │     │   Event Log   │          │
│  │ Anthropic │    │  HTTP     │     │   (Postgres)  │          │
│  │  Ollama   │    │  Git/GH   │     └───────────────┘          │
│  └───────────┘    │  Exec     │                                 │
│                   └───────────┘                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Crate 구조

```
cratos/
├── Cargo.toml                 # Workspace 정의
├── crates/
│   ├── cratos-core/           # 핵심 오케스트레이션
│   │   ├── orchestrator/      # 실행 루프
│   │   ├── planner/           # 계획 수립
│   │   ├── memory/            # 컨텍스트 관리
│   │   └── approval/          # 승인 처리
│   │
│   ├── cratos-channels/       # 채널 어댑터
│   │   ├── telegram/          # teloxide
│   │   ├── slack/             # slack-morphism
│   │   └── discord/           # serenity (future)
│   │
│   ├── cratos-tools/          # 도구 레지스트리
│   │   ├── registry/          # 도구 등록/조회
│   │   ├── runner/            # 실행 엔진
│   │   └── builtins/          # 기본 도구
│   │
│   ├── cratos-llm/            # LLM 프로바이더
│   │   ├── router/            # 모델 라우팅
│   │   ├── openai/            # async-openai
│   │   └── anthropic/         # reqwest
│   │
│   └── cratos-replay/         # 리플레이 엔진
│       ├── event/             # 이벤트 정의
│       ├── store/             # 저장소
│       └── viewer/            # 조회 API
│
├── src/
│   └── main.rs                # CLI 진입점
│
├── config/
│   └── default.toml           # 기본 설정
│
├── migrations/                # DB 마이그레이션
└── tests/                     # 통합 테스트
```

---

## 3. 기능 요구사항 (Functional Requirements)

### 3.1 채널 연동

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| FR-CH-01 | Telegram/Slack에서 메시지 수신 | P0 |
| FR-CH-02 | DM/채널/스레드 구분, 동일 세션 매핑 | P0 |
| FR-CH-03 | 메시지 정규화 (channel, user_id, thread_id, text, attachments) | P0 |
| FR-CH-04 | 응답을 원 요청의 스레드/리플라이 컨텍스트로 전송 | P0 |
| FR-CH-05 | 레이트리밋 준수 및 재시도/백오프 | P1 |

### 3.2 자연어 이해 및 실행 루프

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| FR-NL-01 | 자연어 입력을 실행 계획으로 변환 (목표/단계/도구) | P0 |
| FR-NL-02 | 정보 부족 시 최대 1~3개 질문으로 보완 | P0 |
| FR-NL-03 | 멀티스텝 실행 (도구 호출 → 결과 → 다음 단계) | P0 |
| FR-NL-04 | 실행 단계 수/시간/비용 budget 제한 | P1 |

### 3.3 LLM 연동 및 모델 라우팅

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| FR-LLM-01 | LLM 제공자/모델 설정으로 교체 가능 | P0 |
| FR-LLM-02 | 작업 유형별 모델 자동 선택 | P1 |
| FR-LLM-03 | 구조화된 출력 (JSON schema/함수 호출) | P0 |
| FR-LLM-04 | 스키마 위반 시 자동 교정 재시도 | P1 |

**모델 라우팅 전략:**

```
┌─────────────────┬────────────────┬──────────────┐
│ 작업 유형        │ 모델           │ 비용 수준    │
├─────────────────┼────────────────┼──────────────┤
│ 분류/짧은 요약   │ gpt-4o-mini    │ $ (저)       │
│ 계획 수립        │ gpt-4o/claude  │ $$$ (고)     │
│ 코드 생성        │ gpt-4o/claude  │ $$$ (고)     │
│ 문장 다듬기      │ gpt-4o-mini    │ $ (저)       │
│ 임베딩          │ text-embed-3   │ $ (저)       │
└─────────────────┴────────────────┴──────────────┘
```

### 3.4 메모리/컨텍스트

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| FR-MEM-01 | 세션 키 (channel, workspace, user, thread) 컨텍스트 유지 | P0 |
| FR-MEM-02 | Session Memory (최근 N턴) 저장 | P0 |
| FR-MEM-03 | Working Memory (요약 압축) 자동 생성/갱신 | P1 |
| FR-MEM-04 | Long-term Memory (선호/루틴) 저장/조회 | P2 |
| FR-MEM-05 | "새 작업 시작" 선언 시 컨텍스트 분리 | P1 |

### 3.5 도구(툴) 시스템

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| FR-TL-01 | Tool Registry 제공 | P0 |
| FR-TL-02 | 도구 메타데이터: name, description, input/output schema, risk_level, required_scopes | P0 |
| FR-TL-03 | Tool Runner: 표준 결과 반환 (성공/실패/에러 타입) | P0 |

**최소 제공 도구 (v1):**

| 카테고리 | 도구 | Risk Level |
|----------|------|------------|
| 파일 | list, read, write, move, rename, copy, delete | read/write/destructive |
| 시스템 | exec (allowlist), env_get | write |
| 네트워크 | http_get, http_post | read/write |
| 웹 | fetch_url, extract_text | read |
| 문서 | markdown_create, markdown_save | write |
| Git | clone, fetch, checkout, branch, status, diff, commit, push | write/destructive |
| GitHub | pr_create, pr_comment, ci_status | write |

### 3.6 승인 (Approval)

| 모드 | 설명 |
|------|------|
| `always` | 어떤 도구든 실행 전에 확인 |
| `risky_only` | 파일 변경/외부 전송/코드 푸시 등 위험 작업만 확인 |
| `never` | 묻지 않고 바로 실행 |

승인 요청 메시지 포함 항목:
- 작업 요약 (무엇)
- 영향 범위 (무엇이 바뀜)
- 실행 내용 (가능하면 diff/명령)
- 선택 (승인/취소/수정)

### 3.7 되감기 (Agent Replay)

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| FR-RP-01 | 모든 실행을 리플레이 가능한 이벤트 로그로 저장 | P0 |
| FR-RP-02 | 이벤트: 입력, 계획 요약, LLM 출력, 도구 호출/결과, 최종 응답 | P0 |
| FR-RP-03 | "마지막 실행/특정 실행" 리플레이 요청 | P0 |
| FR-RP-04 | 리플레이 모드: 보기 전용, 동일 입력 재실행, dry-run | P1 |

**리플레이 이벤트 스키마:**

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ReplayEvent {
    pub id: Uuid,
    pub execution_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EventType {
    UserInput,
    PlanCreated,
    LlmRequest,
    LlmResponse,
    ToolCall,
    ToolResult,
    ApprovalRequested,
    ApprovalResponse,
    FinalResponse,
    Error,
}
```

### 3.8 자기 진단 (Tool Doctor)

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| FR-TD-01 | 주요 실패 유형 진단 (권한/토큰/네트워크/레이트리밋/경로 오류) | P1 |
| FR-TD-02 | "원인 후보 + 해결 체크리스트" 제공 | P1 |
| FR-TD-03 | "왜 안 돼?" 질문으로 진단 트리거 | P1 |

### 3.9 원격 개발지시

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| FR-DEV-01 | Git 도구: clone/fetch/checkout/branch/status/diff/commit/push | P0 |
| FR-DEV-02 | 코드 탐색: search/read/list | P0 |
| FR-DEV-03 | 코드 수정: apply_patch/write_file | P0 |
| FR-DEV-04 | 품질 검증: test/lint/typecheck 실행 + 결과 요약 | P1 |
| FR-DEV-05 | GitHub 연동: PR 생성/코멘트/CI 상태 조회 | P1 |
| FR-DEV-06 | 권한 부족 시 안전한 대체 경로 제공 | P1 |
| FR-DEV-07 | 비밀정보 커밋/PR/로그 자동 스캔(마스킹) | P0 |

---

## 4. 비기능 요구사항 (Non-functional Requirements)

### 4.1 보안 (Security)

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| NFR-SEC-01 | 기본 네트워크 노출 최소화 (로컬 바인딩), 외부 노출 시 인증 필수 | P0 |
| NFR-SEC-02 | Slack 서명 검증 등 채널별 검증 | P0 |
| NFR-SEC-03 | 사용자/워크스페이스 allowlist | P1 |
| NFR-SEC-04 | 도구 required_scopes 최소 권한 강제 | P0 |
| NFR-SEC-05 | 민감정보 안전 저장, 로그/응답 마스킹 | P0 |
| NFR-SEC-06 | 원격 개발: force push, 대량 삭제, 비인가 레포 접근 기본 차단 | P0 |
| NFR-SEC-07 | "위험 설정 조합" 감지 경고 | P1 |

### 4.2 신뢰성 (Reliability)

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| NFR-REL-01 | 메시지 수신/전송 재시도/백오프 | P0 |
| NFR-REL-02 | 멱등 처리 (중복 이벤트 방지) | P1 |
| NFR-REL-03 | 도구 실행 타임아웃/중단 가능 | P0 |
| NFR-REL-04 | 부분 실패 시 대안/수동 조치 가이드 | P1 |

### 4.3 성능 (Performance)

| 지표 | 목표 |
|------|------|
| 1차 응답 | 10초 내 (95%) |
| 일반 실행 결과 | 60초 내 (외부 API 지연 제외) |
| 동시 세션 | 50~200 지원 (비동기/큐 기반) |

### 4.4 관측 가능성 (Observability)

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| NFR-OBS-01 | 구조화 로그 (JSON) + trace_id | P0 |
| NFR-OBS-02 | 메트릭: LLM 토큰/비용/지연, 도구 성공률/지연/에러, 채널별 처리량 | P1 |
| NFR-OBS-03 | 실패 케이스 리플레이 형태 보존 | P0 |

### 4.5 유지보수성/확장성

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| NFR-MNT-01 | 채널/LLM/도구 플러그인 교체 가능 | P0 |
| NFR-MNT-02 | 이벤트/도구 스키마 버저닝 | P1 |
| NFR-MNT-03 | 프로젝트별 커맨드 설정 교체 가능 | P1 |

### 4.6 비용/운영

| ID | 요구사항 | 우선순위 |
|----|----------|----------|
| NFR-COST-01 | 모델 라우팅으로 평균 비용 제어 | P1 |
| NFR-COST-02 | 실행 단위별 비용/사용량 리포트 (옵션) | P2 |
| NFR-COST-03 | CPU-only 환경 안정 운영 | P0 |

---

## 5. 기술 스택

| 영역 | 기술 |
|------|------|
| 언어 | Rust 1.75+ |
| 런타임 | Tokio (비동기) |
| 웹 프레임워크 | Axum 0.7 |
| 데이터베이스 | PostgreSQL 16 (sqlx), Redis 7 |
| 채널 | teloxide (Telegram), slack-morphism (Slack) |
| LLM | async-openai (OpenAI), reqwest (Anthropic) |
| 직렬화 | serde, serde_json |
| 설정 | config-rs, dotenvy |
| 로깅 | tracing, tracing-subscriber |
| 테스트 | tokio-test, mockall |

---

## 6. UX 흐름

### 6.1 기본 흐름

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. 사용자가 채널에서 자연어 요청                                  │
│    예: "이 스레드 요약해서 해야 할 일만 뽑아줘"                    │
└─────────────────────┬───────────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. 에이전트 1차 응답 (10초 내 목표)                               │
│    ├─ 바로 답변 가능 → 즉시 답변                                  │
│    ├─ 정보 부족 → 필수 질문만 물어봄                              │
│    └─ 실행 필요 → "지금부터 무엇을 할지" 요약 후 실행 시작         │
└─────────────────────┬───────────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. 실행 (여러 단계 가능)                                         │
│    └─ 승인 필요 시 → 승인 요청 → 사용자 응답 대기                 │
└─────────────────────┬───────────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. 결과 보고 (표준 포맷)                                         │
│    - 무슨 일을 했는지                                            │
│    - 무엇이 바뀌었는지                                           │
│    - 다음에 무엇을 하면 좋은지                                    │
└─────────────────────┬───────────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. 기록 저장 (리플레이/감사 로그)                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 원격 개발지시 흐름

```
사용자: "이 GitHub 이슈 #123 해결해서 PR 만들어줘"
         │
         ▼
┌────────────────────────────────────────────────────┐
│ 1. 레포 준비                                        │
│    - clone/fetch/sync                              │
│    - 브랜치 생성 (issue/123-fix-description)       │
└────────────────────┬───────────────────────────────┘
                     ▼
┌────────────────────────────────────────────────────┐
│ 2. 코드 탐색                                        │
│    - 이슈 내용 분석                                 │
│    - 관련 파일 검색/읽기                            │
└────────────────────┬───────────────────────────────┘
                     ▼
┌────────────────────────────────────────────────────┐
│ 3. 수정                                             │
│    - 패치 생성/적용                                 │
│    - [승인 요청 - risky_only 모드]                 │
└────────────────────┬───────────────────────────────┘
                     ▼
┌────────────────────────────────────────────────────┐
│ 4. 검증                                             │
│    - cargo test / cargo clippy                     │
│    - 타입체크                                       │
└────────────────────┬───────────────────────────────┘
                     ▼
┌────────────────────────────────────────────────────┐
│ 5. 커밋                                             │
│    - Conventional Commits 형식                     │
│    - feat(core): resolve issue #123                │
└────────────────────┬───────────────────────────────┘
                     ▼
┌────────────────────────────────────────────────────┐
│ 6. 푸시 + PR 생성                                   │
│    - [승인 요청 - risky_only 모드]                 │
│    - PR 제목/설명 자동 생성                         │
└────────────────────┬───────────────────────────────┘
                     ▼
┌────────────────────────────────────────────────────┐
│ 7. 요약 보고                                        │
│    - 변경점 요약                                    │
│    - 테스트 결과                                    │
│    - 리스크 체크                                    │
│    - PR 링크                                        │
└────────────────────────────────────────────────────┘
```

---

## 7. 마일스톤

| Phase | 목표 | 주요 기능 |
|-------|------|-----------|
| **Phase 0** | 구조 스캐폴딩 | Workspace 구조, 빈 크레이트, CI 파이프라인 |
| **Phase 1** | 기본 루프 | Telegram 연동, 단일 LLM, 기본 도구, 리플레이 저장 |
| **Phase 2** | 핵심 기능 | 모델 라우팅, 승인 시스템, 리플레이 뷰어 |
| **Phase 3** | 개발지시 | Git/GitHub 도구, 원격 개발 E2E |
| **Phase 4** | 안정화 | Tool Doctor, 메트릭, 비용 리포트 |

---

## Appendix A: 설정 파일 예시

```toml
[server]
host = "127.0.0.1"
port = 8080

[database]
url = "postgres://cratos:cratos@localhost:5432/cratos"
max_connections = 10

[redis]
url = "redis://localhost:6379"

[llm]
default_provider = "openai"

[llm.openai]
api_key = "${OPENAI_API_KEY}"
default_model = "gpt-4o"

[llm.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
default_model = "claude-3-5-sonnet-20241022"

[llm.routing]
classification = "gpt-4o-mini"
planning = "gpt-4o"
code_generation = "claude-3-5-sonnet-20241022"
summarization = "gpt-4o-mini"

[approval]
default_mode = "risky_only"  # always | risky_only | never

[replay]
retention_days = 30
max_events_per_execution = 1000

[channels.telegram]
enabled = true
token = "${TELEGRAM_BOT_TOKEN}"

[channels.slack]
enabled = false
bot_token = "${SLACK_BOT_TOKEN}"
signing_secret = "${SLACK_SIGNING_SECRET}"
```

---

## Appendix B: API 엔드포인트 (Internal)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/webhook/telegram` | Telegram webhook |
| POST | `/api/v1/webhook/slack` | Slack webhook |
| GET | `/api/v1/executions` | 실행 목록 조회 |
| GET | `/api/v1/executions/{id}` | 실행 상세 조회 |
| GET | `/api/v1/executions/{id}/replay` | 리플레이 이벤트 조회 |
| POST | `/api/v1/executions/{id}/rerun` | 재실행 |
| GET | `/api/v1/health` | 헬스체크 |
| GET | `/api/v1/metrics` | 메트릭 (Prometheus 형식) |
