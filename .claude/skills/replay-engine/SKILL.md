---
name: Replay Engine
description: This skill should be used when implementing event sourcing and replay functionality - Cratos key differentiator.
version: 1.0.0
---

# Replay Engine

Cratos 핵심 차별화 기능 - 리플레이 엔진.

## 이벤트 소싱

### Append-only Log

```rust
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, execution_id: Uuid, event: ExecutionEvent) -> Result<()>;
    async fn get_execution(&self, execution_id: Uuid) -> Result<Vec<ExecutionEvent>>;
}
```

### Event Versioning

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "version")]
pub enum ExecutionEvent {
    #[serde(rename = "v1")]
    V1(ExecutionEventV1),
}
```

## 리플레이 모드

### ViewOnly

```rust
pub async fn replay_view_only(
    store: &dyn EventStore,
    execution_id: Uuid,
) -> Result<ReplayResult> {
    let events = store.get_execution(execution_id).await?;
    let timeline = render_timeline(&events);
    Ok(ReplayResult { mode: ViewOnly, timeline, .. })
}
```

### Rerun

```rust
pub async fn replay_rerun(
    store: &dyn EventStore,
    executor: &Executor,
    execution_id: Uuid,
) -> Result<ReplayResult> {
    let events = store.get_execution(execution_id).await?;
    let original_input = extract_input(&events)?;
    let result = executor.execute(Uuid::new_v4(), original_input).await?;
    Ok(result)
}
```

### DryRun

```rust
pub async fn replay_dry_run(
    planner: &Planner,
    execution_id: Uuid,
) -> Result<ReplayResult> {
    let plan = planner.plan(&original_input).await?;
    // 실행 안 함, 계획만 반환
    Ok(ReplayResult { mode: DryRun, plan, .. })
}
```

## 참조

- `.agent/skills/replay-agent/resources/event-schema.md`
- `.agent/skills/replay-agent/resources/execution-protocol.md`
