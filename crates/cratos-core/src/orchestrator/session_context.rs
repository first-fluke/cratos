//! Session context management for the Orchestrator
//!
//! Contains helper methods for loading sessions with RAG context enrichment.

use crate::memory::SessionContext;
use cratos_llm::Message;
use cratos_memory::GraphMemory;
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::config::OrchestratorInput;
use super::core::Orchestrator;

impl Orchestrator {
    /// Load session and enrich with Graph RAG context
    pub(super) async fn load_session_with_context(
        &self,
        session_key: &str,
        input: &OrchestratorInput,
    ) -> Vec<Message> {
        let mut session = match self.memory.get(session_key).await {
            Ok(Some(s)) => {
                debug!(session_key = %session_key, messages = s.get_messages().len(), "Session loaded");
                s
            }
            Ok(None) => {
                debug!(session_key = %session_key, "No existing session, creating new");
                SessionContext::new(session_key)
            }
            Err(e) => {
                warn!(session_key = %session_key, error = %e, "Failed to load session, creating new");
                SessionContext::new(session_key)
            }
        };

        session.add_user_message(&input.text);

        // Graph RAG: always-on context enrichment
        if let Some(gm) = &self.graph_memory {
            self.enrich_with_graph_rag(&mut session, &input.text, gm)
                .await;
            self.inject_explicit_memories(&mut session, &input.text, gm)
                .await;
        }

        // Save updated session
        match self.memory.save(&session).await {
            Ok(()) => {
                debug!(session_key = %session_key, messages = session.get_messages().len(), "Session saved (pre-execution)")
            }
            Err(e) => {
                warn!(session_key = %session_key, error = %e, "Failed to save session (pre-execution)")
            }
        }

        let mut msgs = session.get_messages().to_vec();

        // Attach inline images to the last user message (multimodal support)
        if !input.images.is_empty() {
            if let Some(last_user) = msgs
                .iter_mut()
                .rev()
                .find(|m| m.role == cratos_llm::MessageRole::User)
            {
                last_user.images = input.images.clone();
                info!(
                    image_count = input.images.len(),
                    "Attached {} image(s) to user message",
                    input.images.len()
                );
            }
        }

        msgs
    }

    /// Enrich session with Graph RAG context
    async fn enrich_with_graph_rag(
        &self,
        session: &mut SessionContext,
        query: &str,
        gm: &Arc<GraphMemory>,
    ) {
        let remaining = session.remaining_tokens();
        let total = session.token_count();

        if total > 0 && remaining < session.max_tokens / 5 {
            // Token budget tight: REPLACE middle context
            debug!(
                remaining_tokens = remaining,
                total_tokens = total,
                "Token budget tight, replacing with Graph RAG context"
            );
            let budget = (session.max_tokens / 2) as u32;
            match gm.retrieve(query, 20, budget).await {
                Ok(turns) if !turns.is_empty() => {
                    let retrieved_msgs = GraphMemory::turns_to_messages(&turns);
                    session.replace_with_retrieved(retrieved_msgs);
                    info!(
                        retrieved_turns = turns.len(),
                        "Replaced session context with Graph RAG results"
                    );
                }
                Ok(_) => debug!("No relevant Graph RAG turns found"),
                Err(e) => warn!(error = %e, "Graph RAG retrieval failed"),
            }
        } else {
            // Normal: ADD supplementary context
            let rag_budget = std::cmp::min((session.max_tokens / 10) as u32, 8000);
            match gm.retrieve(query, 5, rag_budget).await {
                Ok(turns) if !turns.is_empty() => {
                    let retrieved_msgs = GraphMemory::turns_to_messages(&turns);
                    session.insert_supplementary_context(retrieved_msgs);
                    debug!(
                        retrieved_turns = turns.len(),
                        "Added supplementary RAG context"
                    );
                }
                Ok(_) => {}
                Err(e) => warn!(error = %e, "Graph RAG supplementary retrieval failed"),
            }
        }
    }

    /// Inject explicit memories into session
    async fn inject_explicit_memories(
        &self,
        session: &mut SessionContext,
        query: &str,
        gm: &Arc<GraphMemory>,
    ) {
        match gm.recall_memories(query, 3).await {
            Ok(memories) if !memories.is_empty() => {
                let memory_names: Vec<&str> = memories.iter().map(|m| m.name.as_str()).collect();
                let memory_context = memories
                    .iter()
                    .map(|m| format!("[Memory: {}] {}", m.name, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");
                session.insert_supplementary_context(vec![Message::system(format!(
                    "Relevant saved memories (use these to help the user):\n{memory_context}"
                ))]);
                info!(
                    count = memories.len(),
                    names = ?memory_names,
                    "Injected explicit memories into context"
                );
            }
            Ok(_) => {
                debug!(query = %query, "No explicit memories matched");
            }
            Err(e) => warn!(error = %e, "Explicit memory recall failed"),
        }
    }
}
