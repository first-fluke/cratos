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
//! - Pantheon: Persona preset system (Olympus OS)
//! - Chronicles: Achievement tracking system (Olympus OS)
//! - Decrees: Laws and rules system (Olympus OS)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod agents;
pub mod approval;
pub mod chronicles;
pub mod credentials;
pub mod decrees;
pub mod error;
pub mod memory;
pub mod olympus_hooks;
pub mod orchestrator;
pub mod pantheon;
pub mod permissions;
pub mod planner;
pub mod scheduler;
pub mod security;
pub mod shutdown;
pub mod telemetry;
pub mod utils;

pub use approval::{ApprovalManager, ApprovalRequest, ApprovalStatus, SharedApprovalManager};
pub use credentials::{
    get_api_key, Credential, CredentialBackend, CredentialError, CredentialStore, SecureString,
};
pub use error::{format_error_for_chat, format_error_for_cli, Error, Result, UserFriendlyError};
pub use memory::{
    MemoryStore, RedisStore, SessionBackend, SessionBackendConfig, SessionContext, SessionStore,
    SqliteStore, ToolExecution, WorkingMemory,
};
pub use orchestrator::{
    ExecutionResult, ExecutionStatus, Orchestrator, OrchestratorConfig, OrchestratorInput,
    ToolCallRecord,
};
pub use permissions::{
    ChannelPermissions, ChannelToolConfig, PermissionConfig, PermissionError, PermissionManager,
    PermissionStatus, TimeRestrictions, ToolPermissions,
};
pub use planner::{PlanResponse, PlanStep, Planner, PlannerConfig};
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

// Re-export shutdown module types
pub use shutdown::{
    shutdown_signal_with_controller, wait_for_shutdown_signal, ShutdownController, ShutdownPhase,
    TaskGuard,
};

// Re-export telemetry module types
pub use telemetry::{
    global_telemetry, init_telemetry, Telemetry, TelemetryConfig, TelemetryEvent, TelemetryStats,
};

// Re-export Olympus hooks
pub use olympus_hooks::{OlympusConfig, OlympusHooks, PostExecutionSummary};

// Re-export pantheon module types (Olympus OS)
pub use pantheon::{
    ActivePersonaState, Domain, PersonaInfo, PersonaLevel, PersonaLoader, PersonaPreset,
    PersonaPrinciples, PersonaSkills, PersonaTraits,
};

// Re-export chronicles module types (Olympus OS)
pub use chronicles::{Chronicle, ChronicleEntry, ChronicleStatus, ChronicleStore, Judgment, Quest};

// Re-export decrees module types (Olympus OS)
pub use decrees::{
    Article, DecreeLoader, EnforcementAction, EnforcerConfig, ExtendedDecreeResult, LawEnforcer,
    LawViolation, Laws, Rank, RankLevel, Ranks, ValidationResult, Warfare, WarfareSection,
};

// Re-export scheduler module types
pub use scheduler::{
    Comparison, CronTrigger, FileEvent, FileTrigger, IntervalTrigger, OneTimeTrigger,
    ScheduledTask, SchedulerConfig, SchedulerEngine, SchedulerEngineBuilder, SchedulerError,
    SchedulerResult, SchedulerStore, SystemMetric, SystemTrigger, TaskAction, TaskExecution,
    TriggerType,
};
