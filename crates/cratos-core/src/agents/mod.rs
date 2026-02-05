//! Multi-Agent System
//!
//! This module provides the unified multi-agent orchestration system for Cratos.
//!
//! ## Design Philosophy
//!
//! Unlike traditional multi-agent systems that require complex CLI parameters,
//! Cratos provides a simple, natural language interface:
//!
//! ```text
//! @backend implement the API
//! @frontend create the UI
//! @qa write tests
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │              Unified Agent Interface                        │
//! │        @backend @frontend @qa @pm @researcher ...          │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    AgentOrchestrator                        │
//! │  ├─ Agent routing (explicit @mention or semantic)          │
//! │  ├─ CLI mapping (each agent → preferred AI CLI)            │
//! │  ├─ Session/workspace auto-management                      │
//! │  └─ Parallel execution + state synchronization             │
//! └─────────────────────────────────────────────────────────────┘
//! ```

mod cli_registry;
mod config;
mod orchestrator;
mod persona_routing;

pub use cli_registry::{CliConfig, CliError, CliProvider, CliRegistry, CliResult};
pub use config::{AgentConfig, AgentPersona, AgentRouting, AgentToolConfig, CliProviderConfig};
pub use orchestrator::{
    AgentOrchestrator, AgentResponse, ExecutionContext, OrchestratorConfig, OrchestratorError,
    OrchestratorResult, ParsedAgentTask, TaskStatus,
};
pub use persona_routing::{domain_to_agent_id, extract_persona_mention, PersonaMapping};
