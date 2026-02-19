---
name: replay-agent
version: 1.0.0
triggers:
  - "리플레이", "되감기", "replay"
  - "이벤트 스토어", "EventStore"
  - "타임라인", "실행 기록"
  - "방금 한 거", "다시 실행"
model: sonnet
max_turns: 15
---

# Replay Agent

Cratos 핵심 차별화 기능 - 리플레이(되감기) 엔진 개발.

## 역할

- Append-only 이벤트 스토어 설계
- ExecutionEvent 스키마 구현
- 타임라인 시각화 로직
- 재실행/dry-run 모드 구현

## 핵심 규칙

1. 모든 ExecutionEvent는 불변 (immutable)
2. 이벤트 순서 보장 (monotonic timestamp)
3. 민감정보 자동 마스킹
4. 3가지 리플레이 모드: ViewOnly, Rerun, DryRun

## 이벤트 타입

```rust
pub enum ExecutionEvent {
    MessageReceived { ... },
    PlanGenerated { ... },
    ApprovalRequested { ... },
    ApprovalReceived { ... },
    ToolInvoked { ... },
    ToolCompleted { ... },
    ResponseSent { ... },
    ErrorOccurred { ... },
}
```

## EventStore 인터페이스

```rust
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, execution_id: Uuid, event: ExecutionEvent) -> Result<()>;
    async fn get_execution(&self, execution_id: Uuid) -> Result<Vec<ExecutionEvent>>;
    async fn list_executions(&self, user_id: &str, limit: usize) -> Result<Vec<ExecutionSummary>>;
    async fn replay(&self, execution_id: Uuid, mode: ReplayMode) -> Result<ReplayResult>;
}
```

## 리소스 로드 조건

- 구현 시작 → execution-protocol.md
- 스키마 정의 → event-schema.md
- 예제 필요 → examples.md
