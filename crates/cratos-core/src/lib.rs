//! Cratos Core - Orchestration Engine
//!
//! This crate provides the core orchestration logic for the Cratos AI assistant,
//! including:
//! - Planning: Converting natural language to execution plans
//! - Execution: Running multi-step tool workflows
//! - Memory: Managing session and working memory contexts
//! - Approval: Handling user approval flows for risky operations
//! - Utils: Retry logic, circuit breaker, and other utilities
//! - Credentials: Secure credential storage
//! - Security: Prompt injection defense

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod agents;
pub mod approval;
pub mod credentials;
pub mod error;
pub mod memory;
pub mod orchestrator;
pub mod permissions;
pub mod planner;
pub mod security;
pub mod utils;

pub use approval::{ApprovalManager, ApprovalRequest, ApprovalStatus, SharedApprovalManager};
pub use credentials::{
    get_api_key, Credential, CredentialBackend, CredentialError, CredentialStore, SecureString,
};
pub use error::{Error, Result};
pub use memory::{
    MemoryStore, RedisStore, SessionBackend, SessionBackendConfig, SessionContext, SessionStore,
    SqliteStore, ToolExecution, WorkingMemory,
};
pub use orchestrator::{
    ExecutionResult, ExecutionStatus, Orchestrator, OrchestratorConfig, OrchestratorInput,
    ToolCallRecord,
};
pub use planner::{PlanResponse, PlanStep, Planner, PlannerConfig};
pub use permissions::{
    ChannelPermissions, ChannelToolConfig, PermissionConfig, PermissionError, PermissionManager,
    PermissionStatus, TimeRestrictions, ToolPermissions,
};
pub use security::{
    sanitize_input, validate_tool_output, InjectionDetector, InjectionError, InjectionPattern,
    SecurityConfig, ThreatLevel,
};
pub use utils::{
    metrics_global, retry_with_backoff, CircuitBreaker, CircuitBreakerConfig, CircuitState,
    Counter, Gauge, Histogram, MetricsRegistry, RateLimitConfig, RateLimitResult, RateLimiter,
    RetryConfig, TieredRateLimiter, Timer,
};

// Re-export agents module types
pub use agents::{
    AgentConfig, AgentOrchestrator, AgentPersona, AgentResponse, AgentRouting, AgentToolConfig,
    CliConfig, CliError, CliProvider, CliProviderConfig, CliRegistry, CliResult, ExecutionContext,
    OrchestratorConfig as AgentOrchestratorConfig, OrchestratorError, OrchestratorResult,
    ParsedAgentTask, TaskStatus,
};
