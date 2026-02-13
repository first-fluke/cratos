//! Query options for listing executions

use crate::event::ExecutionStatus;
use chrono::{DateTime, Utc};

/// Query options for listing executions
#[derive(Debug, Clone, Default)]
pub struct ExecutionQuery {
    /// Filter by channel type
    pub channel_type: Option<String>,
    /// Filter by channel ID
    pub channel_id: Option<String>,
    /// Filter by user ID
    pub user_id: Option<String>,
    /// Filter by status
    pub status: Option<ExecutionStatus>,
    /// Filter by time range (start)
    pub from_time: Option<DateTime<Utc>>,
    /// Filter by time range (end)
    pub to_time: Option<DateTime<Utc>>,
    /// Maximum results
    pub limit: i64,
    /// Offset for pagination
    pub offset: i64,
}

impl ExecutionQuery {
    /// Create a new query with default limits
    #[must_use]
    pub fn new() -> Self {
        Self {
            limit: 50,
            offset: 0,
            ..Default::default()
        }
    }

    /// Set the channel filter
    #[must_use]
    pub fn for_channel(mut self, channel_type: &str, channel_id: &str) -> Self {
        self.channel_type = Some(channel_type.to_string());
        self.channel_id = Some(channel_id.to_string());
        self
    }

    /// Set the user filter
    #[must_use]
    pub fn for_user(mut self, user_id: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }

    /// Set the status filter
    #[must_use]
    pub fn with_status(mut self, status: ExecutionStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Set pagination
    #[must_use]
    pub fn paginate(mut self, limit: i64, offset: i64) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }
}
