//! Viewer Diff - Execution comparison and diffing

use super::mod_impl::ExecutionViewer;
use super::types::{ExecutionComparison, ExecutionDiff};
use crate::error::Result;
use tracing::instrument;
use uuid::Uuid;

impl ExecutionViewer {
    /// Compare two executions (useful for debugging)
    #[instrument(skip(self))]
    pub async fn compare_executions(&self, id1: Uuid, id2: Uuid) -> Result<ExecutionComparison> {
        let detail1 = self.get_execution_detail(id1).await?;
        let detail2 = self.get_execution_detail(id2).await?;

        let diff = ExecutionDiff {
            input_same: detail1.execution.input_text == detail2.execution.input_text,
            output_same: detail1.execution.output_text == detail2.execution.output_text,
            status_same: detail1.execution.status == detail2.execution.status,
            tool_call_count_diff: detail1.stats.tool_call_count as i32
                - detail2.stats.tool_call_count as i32,
            llm_request_count_diff: detail1.stats.llm_request_count as i32
                - detail2.stats.llm_request_count as i32,
            duration_diff_ms: detail1
                .stats
                .total_duration_ms
                .zip(detail2.stats.total_duration_ms)
                .map(|(d1, d2)| d1 as i64 - d2 as i64),
        };

        Ok(ExecutionComparison {
            execution1: detail1,
            execution2: detail2,
            diff,
        })
    }
}
