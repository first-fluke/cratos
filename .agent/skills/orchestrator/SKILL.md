---
name: orchestrator
version: 1.0.0
triggers:
  - "병렬 실행", "parallel"
  - "멀티 에이전트", "orchestrate"
  - "동시 실행"
model: sonnet
max_turns: 25
---

# Orchestrator

CLI 기반 멀티-에이전트 병렬 실행 에이전트.

## 역할

- 여러 에이전트 동시 실행
- 의존성 기반 실행 순서 관리
- 진행 상황 모니터링
- 결과 수집 및 통합

## 핵심 규칙

1. MAX_PARALLEL: 3 (동시 실행 최대)
2. MAX_RETRIES: 2 (재시도 횟수)
3. POLL_INTERVAL: 30s (상태 확인 주기)
4. 의존성 있는 작업은 순차 실행

## 실행 설정

```yaml
orchestration:
  max_parallel: 3
  max_retries: 2
  poll_interval: 30s
  timeout: 600s
```

## 7단계 프로세스

1. **준비**: 스킬 문서 로드
2. **계획 로드**: plan.json 파싱
3. **세션 초기화**: 메모리 파일 생성
4. **에이전트 생성**: 병렬 실행
5. **모니터링**: 상태 폴링
6. **검증**: verify.sh 실행
7. **리포트**: 최종 결과 수집

## 상태 관리

```rust
pub enum AgentStatus {
    Pending,
    Running,
    Completed,
    Failed { error: String, retries: u32 },
}
```

## 리소스 로드 조건

- 병렬 실행 → parallel-execution.md
- 의존성 관리 → dependency-graph.md
