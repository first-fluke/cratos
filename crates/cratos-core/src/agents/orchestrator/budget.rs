use super::types::{OrchestratorError, OrchestratorResult};
use super::AgentOrchestrator;
use std::sync::atomic::Ordering;
use tracing::{debug, warn};

impl AgentOrchestrator {
    /// Add tokens to usage counter, returns error if budget exceeded
    pub(super) fn track_tokens(&self, tokens: u64) -> OrchestratorResult<()> {
        if self.config.token_budget == 0 {
            return Ok(()); // Unlimited
        }

        let new_total = self.tokens_used.fetch_add(tokens, Ordering::SeqCst) + tokens;
        if new_total > self.config.token_budget {
            warn!(
                used = new_total,
                budget = self.config.token_budget,
                "Token budget exceeded"
            );
            return Err(OrchestratorError::BudgetExceeded {
                used: new_total,
                budget: self.config.token_budget,
            });
        }
        debug!(tokens = tokens, total = new_total, "Tokens tracked");
        Ok(())
    }

    /// Check and increment recursion depth
    pub(super) fn enter_depth(&self) -> OrchestratorResult<()> {
        let depth = self.current_depth.fetch_add(1, Ordering::SeqCst) + 1;
        if depth > self.config.max_depth as u64 {
            self.current_depth.fetch_sub(1, Ordering::SeqCst);
            return Err(OrchestratorError::MaxDepthExceeded(self.config.max_depth));
        }
        Ok(())
    }

    /// Decrement recursion depth
    pub(super) fn exit_depth(&self) {
        self.current_depth.fetch_sub(1, Ordering::SeqCst);
    }
}
