use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Query parameters for listing executions
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ListExecutionsQuery {
    /// Maximum number of results
    #[serde(default = "default_limit_inner")]
    pub limit: i64,
    /// Filter by channel type
    pub channel: Option<String>,
    /// Filter by status
    pub status: Option<String>,
    /// Filter by date (from)
    pub from: Option<DateTime<Utc>>,
    /// Filter by date (to)
    pub to: Option<DateTime<Utc>>,
}

pub(crate) fn default_limit_inner() -> i64 {
    50
}

/// Execution summary for list view
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExecutionSummary {
    pub id: Uuid,
    pub channel_type: String,
    pub channel_id: String,
    pub user_id: String,
    pub input_text: String,
    pub output_text: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Detailed execution view
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExecutionDetail {
    pub id: Uuid,
    pub channel_type: String,
    pub channel_id: String,
    pub user_id: String,
    pub thread_id: Option<String>,
    pub input_text: String,
    pub output_text: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub events: Vec<EventSummary>,
}

/// Event summary
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EventSummary {
    pub id: Uuid,
    pub sequence_num: i32,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecutionStats {
    pub labels: Vec<String>,
    pub series: Vec<f64>,
}
