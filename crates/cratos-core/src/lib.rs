//! Cratos Core - Orchestration Engine
//!
//! This crate provides the core orchestration logic for the Cratos AI assistant,
//! including:
//! - Planning: Converting natural language to execution plans
//! - Execution: Running multi-step tool workflows
//! - Memory: Managing session and working memory contexts
//! - Approval: Handling user approval flows for risky operations
//! - Utils: Retry logic, circuit breaker, and other utilities

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod approval;
pub mod error;
pub mod memory;
pub mod orchestrator;
pub mod planner;
pub mod utils;

pub use approval::{ApprovalManager, ApprovalRequest, ApprovalStatus, SharedApprovalManager};
pub use error::{Error, Result};
pub use memory::{
    MemoryStore, RedisStore, SessionContext, SessionStore, ToolExecution, WorkingMemory,
};
pub use orchestrator::{
    ExecutionResult, ExecutionStatus, Orchestrator, OrchestratorConfig, OrchestratorInput,
    ToolCallRecord,
};
pub use planner::{PlanResponse, PlanStep, Planner, PlannerConfig};
pub use utils::{
    metrics_global, retry_with_backoff, CircuitBreaker, CircuitBreakerConfig, CircuitState,
    Counter, Gauge, Histogram, MetricsRegistry, RateLimitConfig, RateLimitResult, RateLimiter,
    RetryConfig, TieredRateLimiter, Timer,
};
