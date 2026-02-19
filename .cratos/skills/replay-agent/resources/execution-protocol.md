# 리플레이 실행 프로토콜

## EventStore 구현

### PostgreSQL 구현

```rust
pub struct PgEventStore {
    pool: PgPool,
}

impl PgEventStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventStore for PgEventStore {
    async fn append(&self, execution_id: Uuid, event: ExecutionEvent) -> Result<()> {
        let event_type = event.type_name();
        let event_data = serde_json::to_value(&event)?;
        let timestamp = event.timestamp();

        sqlx::query!(
            r#"
            INSERT INTO execution_events (execution_id, user_id, event_type, event_data, timestamp)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            execution_id,
            event.user_id(),
            event_type,
            event_data,
            timestamp,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_execution(&self, execution_id: Uuid) -> Result<Vec<ExecutionEvent>> {
        let rows = sqlx::query!(
            r#"
            SELECT event_data FROM execution_events
            WHERE execution_id = $1
            ORDER BY timestamp ASC
            "#,
            execution_id,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| serde_json::from_value(row.event_data))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}
```

## 리플레이 모드 구현

### ViewOnly (조회)

```rust
pub async fn replay_view_only(
    store: &dyn EventStore,
    execution_id: Uuid,
) -> Result<ReplayResult> {
    let events = store.get_execution(execution_id).await?;

    let timeline = events.iter()
        .map(|e| TimelineEntry {
            timestamp: e.timestamp(),
            event_type: e.type_name().to_string(),
            summary: e.summary(),
            details: e.details(),
        })
        .collect();

    Ok(ReplayResult {
        mode: ReplayMode::ViewOnly,
        execution_id,
        timeline,
        rerun_result: None,
    })
}
```

### Rerun (재실행)

```rust
pub async fn replay_rerun(
    store: &dyn EventStore,
    executor: &Executor,
    execution_id: Uuid,
) -> Result<ReplayResult> {
    let events = store.get_execution(execution_id).await?;

    // 원본 입력 추출
    let original_input = events.iter()
        .find_map(|e| match e {
            ExecutionEvent::MessageReceived { message, .. } => Some(message.clone()),
            _ => None,
        })
        .ok_or(Error::NoInputFound)?;

    // 새 실행 ID로 재실행
    let new_execution_id = Uuid::new_v4();
    let result = executor.execute(new_execution_id, original_input).await?;

    Ok(ReplayResult {
        mode: ReplayMode::Rerun,
        execution_id: new_execution_id,
        timeline: result.timeline,
        rerun_result: Some(result),
    })
}
```

### DryRun (시뮬레이션)

```rust
pub async fn replay_dry_run(
    store: &dyn EventStore,
    planner: &Planner,
    execution_id: Uuid,
) -> Result<ReplayResult> {
    let events = store.get_execution(execution_id).await?;

    // 원본 입력 추출
    let original_input = events.iter()
        .find_map(|e| match e {
            ExecutionEvent::MessageReceived { message, .. } => Some(message.clone()),
            _ => None,
        })
        .ok_or(Error::NoInputFound)?;

    // 계획만 생성 (실행 안 함)
    let plan = planner.plan(&original_input).await?;

    let timeline = vec![
        TimelineEntry {
            timestamp: Utc::now(),
            event_type: "DryRun".to_string(),
            summary: "계획만 생성됨 (실행 안 함)".to_string(),
            details: serde_json::to_value(&plan)?,
        },
    ];

    Ok(ReplayResult {
        mode: ReplayMode::DryRun,
        execution_id,
        timeline,
        rerun_result: None,
    })
}
```

## 타임라인 렌더링

```rust
pub fn render_timeline(events: &[ExecutionEvent]) -> String {
    let mut output = String::new();

    output.push_str("┌─────────────────────────────────────────────────────────┐\n");
    output.push_str("│ 🔄 리플레이 타임라인                                    │\n");
    output.push_str("├─────────────────────────────────────────────────────────┤\n");

    let start_time = events.first().map(|e| e.timestamp()).unwrap_or_else(Utc::now);

    for event in events {
        let elapsed = (event.timestamp() - start_time).num_seconds();
        let icon = event.icon();
        let summary = event.summary();

        output.push_str(&format!("│ [{:02}:{:02}] {} {}\n",
            elapsed / 60, elapsed % 60, icon, summary));
    }

    output.push_str("└─────────────────────────────────────────────────────────┘\n");
    output
}
```
