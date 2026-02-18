//! Result building helpers for the Orchestrator
//!
//! Contains helper methods for building ExecutionResult for various termination conditions.

use super::core::Orchestrator;
use super::sanitize::sanitize_error_for_user;
use super::types::{ExecutionResult, ExecutionStatus, ToolCallRecord};
use crate::event_bus::OrchestratorEvent;
use cratos_replay::EventType;
use tracing::{error, warn};
use uuid::Uuid;

impl Orchestrator {
    /// Build an ExecutionResult for planning failure
    pub(super) async fn build_planning_failure_result(
        &self,
        execution_id: Uuid,
        error: &crate::error::Error,
        tool_call_records: Vec<ToolCallRecord>,
        iteration: usize,
        duration_ms: u64,
        model_used: Option<String>,
    ) -> ExecutionResult {
        error!(execution_id = %execution_id, error = %error, "Planning failed");

        self.log_event(
            execution_id,
            EventType::Error,
            &serde_json::json!({
                "error": error.to_string(),
                "phase": "planning"
            }),
        )
        .await;

        let user_msg = self.error_to_user_message(error);
        let error_detail = format!("Error: {}", error);

        self.emit(OrchestratorEvent::ExecutionFailed {
            execution_id,
            error: error_detail.clone(),
        });

        // Update execution status in DB
        if let Some(store) = &self.event_store {
            let _ = store
                .update_execution_status(execution_id, "failed", Some(&error_detail))
                .await;
        }

        crate::utils::metrics_global::labeled_counter("cratos_executions_total")
            .inc(&[("status", "failed")]);
        crate::utils::metrics_global::gauge("cratos_active_executions").dec();

        ExecutionResult {
            execution_id,
            status: ExecutionStatus::Failed,
            response: user_msg,
            tool_calls: tool_call_records,
            artifacts: Vec::new(),
            iterations: iteration,
            duration_ms,
            model: model_used,
        }
    }

    /// Convert an error to a user-friendly message
    pub(super) fn error_to_user_message(&self, error: &crate::error::Error) -> String {
        match error {
            crate::error::Error::Llm(cratos_llm::Error::RateLimit) => {
                "요청이 너무 많습니다. 잠시 후 다시 시도해주세요.".to_string()
            }
            crate::error::Error::Llm(cratos_llm::Error::Api(api_err))
                if api_err.contains("INVALID_ARGUMENT") =>
            {
                warn!(
                    "Gemini INVALID_ARGUMENT (likely function call/response mismatch): {}",
                    api_err
                );
                "내부 처리 오류가 발생했습니다. 다시 시도해주세요.".to_string()
            }
            crate::error::Error::Llm(cratos_llm::Error::Api(api_err))
                if api_err.contains("authentication") || api_err.contains("401") =>
            {
                "API 인증 오류가 발생했습니다. 관리자에게 문의해주세요.".to_string()
            }
            crate::error::Error::Llm(cratos_llm::Error::ServerError(_)) => {
                "AI 서버에 일시적 장애가 발생했습니다. 잠시 후 다시 시도해주세요.".to_string()
            }
            _ => {
                let raw: String = error.to_string().chars().take(80).collect();
                format!(
                    "오류가 발생했습니다. 다시 시도해주세요. ({})",
                    sanitize_error_for_user(&raw)
                )
            }
        }
    }

    /// Build an ExecutionResult for cancelled execution
    pub(super) fn build_cancelled_result(
        &self,
        execution_id: Uuid,
        reason: Option<String>,
        tool_call_records: Vec<ToolCallRecord>,
        iteration: usize,
        duration_ms: u64,
        model_used: Option<String>,
    ) -> ExecutionResult {
        ExecutionResult {
            execution_id,
            status: ExecutionStatus::Cancelled,
            response: reason.unwrap_or_else(|| "실행이 취소되었습니다.".to_string()),
            tool_calls: tool_call_records,
            artifacts: Vec::new(),
            iterations: iteration,
            duration_ms,
            model: model_used,
        }
    }
}
