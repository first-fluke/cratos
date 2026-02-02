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

## 데이터베이스 스키마

```sql
CREATE TABLE execution_events (
    id BIGSERIAL PRIMARY KEY,
    execution_id UUID NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    event_type VARCHAR(50) NOT NULL,
    event_data JSONB NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_execution_events_execution_id ON execution_events(execution_id);
CREATE INDEX idx_execution_events_user_id ON execution_events(user_id);
CREATE INDEX idx_execution_events_timestamp ON execution_events(timestamp);
```
