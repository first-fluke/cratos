# 이벤트 스키마 정의

## ExecutionEvent Enum

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExecutionEvent {
    // 입력 수신
    MessageReceived {
        message: NormalizedMessage,
        timestamp: DateTime<Utc>,
    },

    // 계획 생성
    PlanGenerated {
        plan: ExecutionPlan,
        model_used: String,
        tokens_used: u32,
        timestamp: DateTime<Utc>,
    },

    // 승인 요청
    ApprovalRequested {
        action: String,
        risk_level: RiskLevel,
        details: ApprovalDetails,
        timestamp: DateTime<Utc>,
    },

    // 승인 응답
    ApprovalReceived {
        approved: bool,
        modified: Option<String>,
        timestamp: DateTime<Utc>,
    },

    // 도구 호출 시작
    ToolInvoked {
        tool: String,
        input: serde_json::Value,
        input_masked: serde_json::Value, // 민감정보 마스킹
        timestamp: DateTime<Utc>,
    },

    // 도구 호출 완료
    ToolCompleted {
        tool: String,
        output: ToolResult,
        output_masked: ToolResult, // 민감정보 마스킹
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    // 응답 전송
    ResponseSent {
        text: String,
        channel: Channel,
        timestamp: DateTime<Utc>,
    },

    // 에러 발생
    ErrorOccurred {
        error: String,
        error_type: ErrorType,
        recoverable: bool,
        timestamp: DateTime<Utc>,
    },
}
```

## 보조 타입

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub goal: String,
    pub steps: Vec<PlanStep>,
    pub estimated_risk: RiskLevel,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub description: String,
    pub tool: String,
    pub input: serde_json::Value,
    pub depends_on: Vec<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RiskLevel {
    Read,
    Write,
    Destructive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: serde_json::Value,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorType {
    Network,
    Permission,
    Timeout,
    Validation,
    Internal,
}
```

## 민감정보 마스킹

```rust
const SENSITIVE_KEYS: &[&str] = &[
    "password", "token", "api_key", "secret",
    "credential", "auth", "bearer",
];

pub fn mask_sensitive(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut masked = serde_json::Map::new();
            for (k, v) in map {
                let is_sensitive = SENSITIVE_KEYS.iter()
                    .any(|s| k.to_lowercase().contains(s));

                if is_sensitive {
                    masked.insert(k.clone(), serde_json::Value::String("***MASKED***".into()));
                } else {
                    masked.insert(k.clone(), mask_sensitive(v));
                }
            }
            serde_json::Value::Object(masked)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(mask_sensitive).collect())
        }
        _ => value.clone(),
    }
}
```

## 데이터베이스 스키마 (SQLite)

> **중요**: Cratos는 PostgreSQL이 아닌 **SQLite**를 사용합니다.

```sql
-- 실행 기록 테이블
CREATE TABLE IF NOT EXISTS executions (
    id TEXT PRIMARY KEY,                    -- UUID
    user_id TEXT NOT NULL,
    channel TEXT NOT NULL,
    input_text TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, running, completed, failed
    created_at TEXT NOT NULL,               -- ISO 8601
    completed_at TEXT
);

-- 이벤트 테이블
CREATE TABLE IF NOT EXISTS execution_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL,               -- JSON 문자열
    timestamp TEXT NOT NULL,                -- ISO 8601
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (execution_id) REFERENCES executions(id)
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_events_execution_id ON execution_events(execution_id);
CREATE INDEX IF NOT EXISTS idx_events_user_id ON execution_events(user_id);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON execution_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_executions_user_id ON executions(user_id);
CREATE INDEX IF NOT EXISTS idx_executions_created_at ON executions(created_at);
```

## sqlx 쿼리 예시

```rust
use sqlx::SqlitePool;

// 이벤트 저장
pub async fn insert_event(
    pool: &SqlitePool,
    execution_id: &str,
    user_id: &str,
    event: &ExecutionEvent,
) -> Result<i64> {
    let event_type = event.event_type_name();
    let event_data = serde_json::to_string(event)?;
    let timestamp = Utc::now().to_rfc3339();

    let result = sqlx::query!(
        r#"
        INSERT INTO execution_events (execution_id, user_id, event_type, event_data, timestamp)
        VALUES (?, ?, ?, ?, ?)
        "#,
        execution_id,
        user_id,
        event_type,
        event_data,
        timestamp
    )
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

// 실행 기록 조회
pub async fn get_events(
    pool: &SqlitePool,
    execution_id: &str,
) -> Result<Vec<ExecutionEvent>> {
    let rows = sqlx::query!(
        r#"
        SELECT event_data FROM execution_events
        WHERE execution_id = ?
        ORDER BY timestamp ASC
        "#,
        execution_id
    )
    .fetch_all(pool)
    .await?;

    rows.iter()
        .map(|r| serde_json::from_str(&r.event_data))
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}
```

## 데이터 위치

- 이벤트 DB: `~/.cratos/cratos.db`
- 스킬 DB: `~/.cratos/skills.db`
- 메모리 DB: `~/.cratos/memory.db`
