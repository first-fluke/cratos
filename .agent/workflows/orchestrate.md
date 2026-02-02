---
name: orchestrate
description: CLI 기반 멀티-에이전트 병렬 실행
triggers:
  - "/orchestrate"
---

# /orchestrate - 멀티-에이전트 병렬 실행

## 설정

```yaml
MAX_PARALLEL: 3
MAX_RETRIES: 2
POLL_INTERVAL: 30s
TIMEOUT: 600s
```

## 7단계 프로세스

### Step 0: 준비

```
1. 스킬 문서 로드
   - .agent/skills/**/SKILL.md 읽기
   - 트리거 키워드 추출

2. 규칙 확인
   - rust-conventions.md
   - memory-protocol.md
```

### Step 1: 계획 로드

```
1. .agent/plan.json 확인
   - 없으면 pm-agent 호출하여 생성

2. 계획 파싱
   - 작업 목록 추출
   - 의존성 그래프 생성
```

### Step 2: 세션 초기화

```
1. 세션 ID 생성
   - UUID v4

2. 메모리 파일 생성
   - .serena/memories/session-{id}.md
```

### Step 3: 에이전트 생성

```
1. 의존성 없는 작업 식별
2. MAX_PARALLEL 만큼 동시 실행
3. 각 에이전트에게 작업 할당
```

### Step 4: 진행 상황 모니터링

```
POLL_INTERVAL (30초) 마다:
1. 각 에이전트 상태 확인
2. 완료된 에이전트 결과 수집
3. 새 작업 할당 (의존성 해소된 것)
4. 터미널 대시보드 업데이트
```

### Step 5: 검증

```
각 에이전트 완료 후:
1. verify.sh 실행
2. 실패 시 재시도 (MAX_RETRIES)
3. 최종 실패 시 사용자 알림
```

### Step 6-7: 결과 수집 & 리포트

```
1. 모든 에이전트 결과 수집
2. 메모리 파일 읽기
3. 최종 리포트 생성
```

## 대시보드 출력 예시

```
┌─────────────────────────────────────────────────────────────┐
│ 🚀 Orchestrator Dashboard                                   │
├─────────────────────────────────────────────────────────────┤
│ Session: abc123 | Elapsed: 2m 30s                          │
├─────────────────────────────────────────────────────────────┤
│ [1/5] ✅ rust-agent: 코드 수정 완료                         │
│ [2/5] ✅ qa-agent: 테스트 통과                              │
│ [3/5] ⏳ commit: PR 생성 중...                              │
│ [4/5] ⏸️ infra-agent: 대기 중 (depends: 3)                  │
│ [5/5] ⏸️ docs-agent: 대기 중 (depends: 3)                   │
└─────────────────────────────────────────────────────────────┘
```
