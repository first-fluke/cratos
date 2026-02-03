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
//! @backend API 구현해줘
//! @frontend UI 만들어줘
//! @qa 테스트 작성해줘
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    통합 에이전트 인터페이스                    │
//! │        @backend @frontend @qa @pm @researcher ...          │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    AgentOrchestrator                        │
//! │  ├─ 에이전트 라우팅 (명시적 @멘션 or 시맨틱)                   │
//! │  ├─ CLI 매핑 (각 에이전트 → 선호 AI CLI)                      │
//! │  ├─ 세션/워크스페이스 자동 관리                               │
//! │  └─ 병렬 실행 + 상태 동기화                                   │
//! └─────────────────────────────────────────────────────────────┘
//! ```

mod cli_registry;
mod config;
mod orchestrator;

pub use cli_registry::{CliConfig, CliError, CliProvider, CliRegistry, CliResult};
pub use config::{AgentConfig, AgentPersona, AgentRouting, AgentToolConfig, CliProviderConfig};
pub use orchestrator::{
    AgentOrchestrator, AgentResponse, ExecutionContext, OrchestratorConfig, OrchestratorError,
    OrchestratorResult, ParsedAgentTask, TaskStatus,
};
