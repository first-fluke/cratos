//! Orchestrator core structure
//!
//! Contains the main `Orchestrator` struct and its builder methods.

use crate::agents::PersonaMapping;
use crate::approval::SharedApprovalManager;
use crate::event_bus::{EventBus, OrchestratorEvent};
use crate::memory::{MemoryStore, SessionStore};
use crate::olympus_hooks::OlympusHooks;
use crate::planner::Planner;
use crate::tool_policy::ToolSecurityPolicy;
use cratos_llm::LlmProvider;
use cratos_memory::GraphMemory;
use cratos_replay::EventStoreTrait;
use cratos_tools::{ToolDoctor, ToolRegistry, ToolRunner};
use dashmap::DashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;
use uuid::Uuid;

use super::config::OrchestratorConfig;
use super::types::SkillRouting;

/// Main orchestrator that coordinates execution
pub struct Orchestrator {
    pub(crate) planner: Planner,
    pub(crate) runner: ToolRunner,
    pub(crate) memory: Arc<dyn SessionStore>,
    pub(crate) event_store: Option<Arc<dyn EventStoreTrait>>,
    pub(crate) event_bus: Option<Arc<EventBus>>,
    pub(crate) approval_manager: Option<SharedApprovalManager>,
    pub(crate) olympus_hooks: Option<OlympusHooks>,
    pub(crate) graph_memory: Option<Arc<GraphMemory>>,
    pub(crate) fallback_planner: Option<Planner>,
    pub(crate) persona_mapping: Option<PersonaMapping>,
    pub(crate) skill_router: Option<Arc<dyn SkillRouting>>,
    pub(crate) security_policy: Option<ToolSecurityPolicy>,
    /// Persona-skill binding store for tracking persona-specific skill metrics
    pub(crate) persona_skill_store: Option<Arc<cratos_skills::PersonaSkillStore>>,
    /// Chronicle store for tracking persona quests and history
    pub(crate) chronicle_store: Option<Arc<crate::chronicles::ChronicleStore>>,
    pub(crate) doctor: ToolDoctor,
    pub(crate) config: OrchestratorConfig,
    /// Active executions with cancellation tokens for chat.cancel support
    pub(crate) active_executions: Arc<DashMap<Uuid, CancellationToken>>,
}

impl Orchestrator {
    /// Create a new orchestrator
    #[must_use]
    pub fn new(
        llm_provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        config: OrchestratorConfig,
    ) -> Self {
        let planner = Planner::new(llm_provider, config.planner_config.clone());
        let runner = ToolRunner::new(tool_registry, config.runner_config.clone());

        Self {
            planner,
            runner,
            memory: Arc::new(MemoryStore::new()),
            event_store: None,
            event_bus: None,
            approval_manager: None,
            olympus_hooks: None,
            graph_memory: None,
            fallback_planner: None,
            persona_mapping: None,
            skill_router: None,
            security_policy: None,
            persona_skill_store: None,
            chronicle_store: None,
            doctor: ToolDoctor::new(),
            config,
            active_executions: Arc::new(DashMap::new()),
        }
    }

    /// Set the event store for logging
    pub fn with_event_store(mut self, store: Arc<dyn EventStoreTrait>) -> Self {
        self.event_store = Some(store);
        self
    }

    /// Set the event bus for real-time event broadcasting
    pub fn with_event_bus(mut self, bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Get a reference to the event bus (if set)
    #[must_use]
    pub fn event_bus(&self) -> Option<&Arc<EventBus>> {
        self.event_bus.as_ref()
    }

    /// Set the memory store
    pub fn with_memory(mut self, memory: Arc<dyn SessionStore>) -> Self {
        self.memory = memory;
        self
    }

    /// Set the approval manager for high-risk tool execution
    pub fn with_approval_manager(mut self, manager: SharedApprovalManager) -> Self {
        self.approval_manager = Some(manager);
        self
    }

    /// Set the Olympus OS hooks for post-execution processing
    pub fn with_olympus_hooks(mut self, hooks: OlympusHooks) -> Self {
        self.olympus_hooks = Some(hooks);
        self
    }

    /// Set the Graph RAG memory for long-term context retrieval
    pub fn with_graph_memory(mut self, graph_memory: Arc<GraphMemory>) -> Self {
        self.graph_memory = Some(graph_memory);
        self
    }

    /// Set the fallback LLM provider for rate-limit recovery
    pub fn with_fallback_provider(mut self, provider: Arc<dyn LlmProvider>) -> Self {
        let fallback_config = self.config.planner_config.clone();
        self.fallback_planner = Some(Planner::new(provider, fallback_config));
        self
    }

    /// Set the persona mapping for @mention and LLM-based routing
    pub fn with_persona_mapping(mut self, mapping: PersonaMapping) -> Self {
        self.persona_mapping = Some(mapping);
        self
    }

    /// Set the skill router for semantic skill matching
    pub fn with_skill_router(mut self, router: Arc<dyn SkillRouting>) -> Self {
        self.skill_router = Some(router);
        self
    }

    /// Set the 6-level tool security policy
    pub fn with_security_policy(mut self, policy: ToolSecurityPolicy) -> Self {
        self.security_policy = Some(policy);
        self
    }

    /// Set the persona-skill store for tracking persona-specific skill metrics
    pub fn with_persona_skill_store(
        mut self,
        store: Arc<cratos_skills::PersonaSkillStore>,
    ) -> Self {
        self.persona_skill_store = Some(store);
        self
    }

    /// Set the chronicle store for tracking persona quests
    pub fn with_chronicle_store(mut self, store: Arc<crate::chronicles::ChronicleStore>) -> Self {
        self.chronicle_store = Some(store);
        self
    }

    /// Get the tool runner
    #[must_use]
    pub fn runner(&self) -> &ToolRunner {
        &self.runner
    }

    /// Get the approval manager (if set)
    #[must_use]
    pub fn approval_manager(&self) -> Option<&SharedApprovalManager> {
        self.approval_manager.as_ref()
    }

    /// Get the active executions map (for chat.cancel support)
    #[must_use]
    pub fn active_executions(&self) -> &Arc<DashMap<Uuid, CancellationToken>> {
        &self.active_executions
    }

    /// Get the number of active executions
    #[must_use]
    pub fn active_execution_count(&self) -> Option<usize> {
        Some(self.active_executions.len())
    }

    /// Get the LLM provider name
    #[must_use]
    pub fn provider_name(&self) -> &str {
        self.planner.provider().name()
    }

    /// Get available LLM models
    #[must_use]
    pub fn available_models(&self) -> Vec<String> {
        self.planner.provider().available_models()
    }

    /// List all registered tool names
    #[must_use]
    pub fn list_tool_names(&self) -> Vec<String> {
        self.runner
            .registry()
            .list_names()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Cancel an active execution by ID
    pub fn cancel_execution(&self, execution_id: Uuid) -> bool {
        if let Some((_id, token)) = self.active_executions.remove(&execution_id) {
            token.cancel();
            self.emit(OrchestratorEvent::ExecutionCancelled { execution_id });
            info!(execution_id = %execution_id, "Execution cancelled");
            true
        } else {
            false
        }
    }
}
