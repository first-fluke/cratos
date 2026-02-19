use serde::Serialize;
use uuid::Uuid;

/// Events emitted during orchestrator execution.
///
/// **Security note**: These events intentionally exclude sensitive data (API keys,
/// full tool outputs, etc.). Detailed data should be fetched via authenticated
/// REST endpoints using the `execution_id`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorEvent {
    /// Execution has started
    ExecutionStarted {
        /// Unique execution identifier
        execution_id: Uuid,
        /// Session key for this execution
        session_key: String,
    },
    /// LLM planning step started
    PlanningStarted {
        /// Execution identifier
        execution_id: Uuid,
        /// Current iteration number
        iteration: usize,
    },
    /// Streaming text delta from LLM
    ChatDelta {
        /// Execution identifier
        execution_id: Uuid,
        /// Text chunk
        delta: String,
        /// Whether this is the final chunk
        is_final: bool,
    },
    /// Tool execution started
    ToolStarted {
        /// Execution identifier
        execution_id: Uuid,
        /// Name of the tool being executed
        tool_name: String,
        /// Tool call ID from the LLM
        tool_call_id: String,
    },
    /// Tool execution completed
    ToolCompleted {
        /// Execution identifier
        execution_id: Uuid,
        /// Tool call ID
        tool_call_id: String,
        /// Name of the tool that completed
        tool_name: String,
        /// Whether the tool succeeded
        success: bool,
        /// Execution duration in milliseconds
        duration_ms: u64,
    },
    /// User approval is required
    ApprovalRequired {
        /// Execution identifier
        execution_id: Uuid,
        /// Approval request ID
        request_id: Uuid,
    },
    /// Execution completed successfully
    ExecutionCompleted {
        /// Execution identifier
        execution_id: Uuid,
    },
    /// Execution failed
    ExecutionFailed {
        /// Execution identifier
        execution_id: Uuid,
        /// Error description (sanitized, no sensitive data)
        error: String,
    },
    /// Execution was cancelled
    ExecutionCancelled {
        /// Execution identifier
        execution_id: Uuid,
    },
    /// A2A message sent between agents
    A2aMessageSent {
        /// Session identifier
        session_id: String,
        /// Sending agent ID
        from_agent: String,
        /// Receiving agent ID
        to_agent: String,
        /// Message ID
        message_id: Uuid,
    },
    /// Provider quota approaching limit
    ///
    /// Emitted when a provider's remaining quota falls below threshold (default 20%).
    QuotaWarning {
        /// Provider name (e.g., "openai", "anthropic", "gemini")
        provider: String,
        /// Remaining percentage (0.0 - 100.0)
        remaining_pct: f64,
        /// Seconds until quota resets (if known)
        reset_in_secs: Option<i64>,
    },
}

impl OrchestratorEvent {
    /// Get the execution_id from any event variant.
    ///
    /// Returns a placeholder UUID for events without execution context.
    #[must_use]
    pub fn execution_id(&self) -> Uuid {
        match self {
            Self::ExecutionStarted { execution_id, .. }
            | Self::PlanningStarted { execution_id, .. }
            | Self::ChatDelta { execution_id, .. }
            | Self::ToolStarted { execution_id, .. }
            | Self::ToolCompleted { execution_id, .. }
            | Self::ApprovalRequired { execution_id, .. }
            | Self::ExecutionCompleted { execution_id }
            | Self::ExecutionFailed { execution_id, .. }
            | Self::ExecutionCancelled { execution_id } => *execution_id,
            Self::A2aMessageSent { message_id, .. } => *message_id,
            // QuotaWarning has no execution context
            Self::QuotaWarning { .. } => Uuid::nil(),
        }
    }
}
