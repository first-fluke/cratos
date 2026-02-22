//! Orchestrator planning methods
//!
//! Contains planning-related methods for the Orchestrator:
//! - `dispatch_plan`: Dispatches a plan step to a planner
//! - `plan_with_fallback`: Plans with automatic fallback on transient errors
//! - `try_final_summary`: Generates final summary when limits are reached

use crate::planner::{PlanResponse, Planner};
use cratos_llm::{Message, ToolDefinition};
use tracing::warn;

use super::core::Orchestrator;
use super::sanitize::is_fallback_eligible;

impl Orchestrator {
    /// Dispatch a plan step to the given planner with an optional system prompt override.
    ///
    /// Wraps the LLM call in a 120-second timeout to prevent indefinite hangs
    /// when a provider fails to respond (e.g. network stall, missing HTTP timeout).
    pub(crate) async fn dispatch_plan(
        planner: &Planner,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt_override: Option<&str>,
        override_model: Option<&str>,
    ) -> crate::error::Result<PlanResponse> {
        let fut = async move {
            match system_prompt_override {
                Some(p) => {
                    planner
                        .plan_step_with_system_prompt(messages, tools, p, override_model)
                        .await
                }
                None => planner.plan_step(messages, tools, override_model).await,
            }
        };
        match tokio::time::timeout(std::time::Duration::from_secs(120), fut).await {
            Ok(result) => result,
            Err(_) => {
                warn!("LLM dispatch timed out after 120s");
                Err(crate::error::Error::from(cratos_llm::Error::Timeout(
                    120_000,
                )))
            }
        }
    }

    /// Plan a step with automatic fallback on transient errors.
    ///
    /// When `fallback_sticky` is `true`, the fallback planner is used directly
    /// (skipping the primary).  This prevents mixing tool calls from different
    /// providers within the same execution — critical for Gemini 3 thinking
    /// models that require `thought_signature` on every function call.
    pub(crate) async fn plan_with_fallback(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt_override: Option<&str>,
        override_model: Option<&str>,
        fallback_sticky: &mut bool,
    ) -> crate::error::Result<PlanResponse> {
        // If a previous iteration already fell back, keep using the fallback
        // to avoid mixing thought_signature-bearing and bare function calls.
        if *fallback_sticky {
            if let Some(fb) = self.fallback_planner.as_ref() {
                return Self::dispatch_plan(fb, messages, tools, system_prompt_override, None).await;
            }
        }

        let result =
            Self::dispatch_plan(&self.planner, messages, tools, system_prompt_override, override_model).await;
        match result {
            Ok(resp) => Ok(resp),
            Err(ref e) if self.fallback_planner.is_some() && is_fallback_eligible(e) => {
                warn!(error = %e, "Primary provider failed, trying fallback (sticky)");
                *fallback_sticky = true;
                let fb = self.fallback_planner.as_ref().unwrap();
                Self::dispatch_plan(fb, messages, tools, system_prompt_override, None).await
            }
            Err(e) => Err(e),
        }
    }

    /// Generate a final summary when iterations or timeout are exhausted.
    ///
    /// Makes one LLM call **without tools** so the model must produce a
    /// text answer from whatever context has accumulated in `messages`.
    pub(crate) async fn try_final_summary(
        &self,
        messages: &[Message],
        system_prompt_override: Option<&str>,
        override_model: Option<&str>,
        fallback_sticky: bool,
    ) -> String {
        // Nothing useful to summarize if conversation is trivially short
        if messages.len() <= 2 {
            return String::new();
        }

        let mut summary_messages = messages.to_vec();
        summary_messages.push(Message::user(
            "지금까지의 도구 실행 결과를 바탕으로 최종 답변을 생성해주세요. \
             더 이상 도구를 사용하지 말고, 현재까지 수집한 정보로 가능한 한 \
             도움이 되는 답변을 해주세요.",
        ));

        let planner = if fallback_sticky {
            self.fallback_planner.as_ref().unwrap_or(&self.planner)
        } else {
            &self.planner
        };

        let result = Self::dispatch_plan(
            planner,
            &summary_messages,
            &[], // empty tools → forces text-only response
            system_prompt_override,
            override_model,
        )
        .await;

        match result {
            Ok(resp) => resp.content.unwrap_or_default(),
            Err(e) => {
                warn!(error = %e, "Final summary generation failed");
                String::new()
            }
        }
    }

}
