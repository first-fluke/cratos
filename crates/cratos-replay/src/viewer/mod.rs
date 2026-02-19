//! Viewer - Event query and replay API
//!
//! This module provides the query and visualization layer for the replay system.
//! It transforms raw events into human-readable timelines and summaries.

pub mod types;
pub mod timeline;
pub mod stats;
pub mod replay;
pub mod diff;

// Internal module for partial impls to refer to
pub(crate) mod mod_impl {
    pub use super::ExecutionViewer;
}

use crate::error::Result;
use crate::event::{EventType, TimelineEntry};
use crate::store::EventStore;
use chrono::{DateTime, Utc};
use tracing::instrument;
use uuid::Uuid;

pub use types::{
    EventChain, ExecutionComparison, ExecutionDetail, ExecutionDiff, ExecutionStats,
    ExecutionSummary, ReplayOptions, ReplayResult, ReplayStep,
};

#[cfg(test)]
mod tests;

/// Viewer for querying and displaying execution history
#[derive(Clone)]
pub struct ExecutionViewer {
    pub(crate) store: EventStore,
}

impl ExecutionViewer {
    /// Create a new execution viewer
    #[must_use]
    pub fn new(store: EventStore) -> Self {
        Self { store }
    }

    /// Get a detailed view of an execution with its timeline
    #[instrument(skip(self))]
    pub async fn get_execution_detail(&self, execution_id: Uuid) -> Result<ExecutionDetail> {
        let execution = self.store.get_execution(execution_id).await?;
        let events = self.store.get_execution_events(execution_id).await?;

        let timeline = self.build_timeline(&events);
        let summary = self.build_summary(&execution, &events);
        let stats = self.calculate_stats(&events);

        Ok(ExecutionDetail {
            execution,
            timeline,
            summary,
            stats,
        })
    }

    /// Get a simplified timeline for an execution
    #[instrument(skip(self))]
    pub async fn get_timeline(&self, execution_id: Uuid) -> Result<Vec<TimelineEntry>> {
        let events = self.store.get_execution_events(execution_id).await?;
        Ok(self.build_timeline(&events))
    }

    /// Get execution statistics
    #[instrument(skip(self))]
    pub async fn get_stats(&self, execution_id: Uuid) -> Result<ExecutionStats> {
        let events = self.store.get_execution_events(execution_id).await?;
        Ok(self.calculate_stats(&events))
    }

    /// Search executions by input text
    #[instrument(skip(self))]
    pub async fn search_executions(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<ExecutionSummary>> {
        // For now, just get recent executions and filter
        // In production, this would use full-text search
        let executions = self.store.list_recent_executions(limit * 2).await?;

        let filtered: Vec<_> = executions
            .into_iter()
            .filter(|e| e.input_text.to_lowercase().contains(&query.to_lowercase()))
            .take(limit as usize)
            .collect();

        let mut summaries = Vec::with_capacity(filtered.len());
        for execution in filtered {
            let events = self.store.get_execution_events(execution.id).await?;
            let summary = self.build_summary(&execution, &events);
            summaries.push(summary);
        }

        Ok(summaries)
    }

    /// Get recent execution summaries
    #[instrument(skip(self))]
    pub async fn get_recent_summaries(&self, limit: i64) -> Result<Vec<ExecutionSummary>> {
        let executions = self.store.list_recent_executions(limit).await?;

        let mut summaries = Vec::with_capacity(executions.len());
        for execution in executions {
            let events = self.store.get_execution_events(execution.id).await?;
            let summary = self.build_summary(&execution, &events);
            summaries.push(summary);
        }

        Ok(summaries)
    }

    /// Get executions in a time range
    #[instrument(skip(self))]
    pub async fn get_executions_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        channel_type: Option<&str>,
        channel_id: Option<&str>,
    ) -> Result<Vec<ExecutionSummary>> {
        // Get recent executions and filter by time range
        let executions = self.store.list_recent_executions(1000).await?;

        let filtered: Vec<_> = executions
            .into_iter()
            .filter(|e| {
                e.created_at >= from
                    && e.created_at <= to
                    && channel_type.is_none_or(|ct| e.channel_type == ct)
                    && channel_id.is_none_or(|ci| e.channel_id == ci)
            })
            .collect();

        let mut summaries = Vec::with_capacity(filtered.len());
        for execution in filtered {
            let events = self.store.get_execution_events(execution.id).await?;
            let summary = self.build_summary(&execution, &events);
            summaries.push(summary);
        }

        Ok(summaries)
    }

    /// Get the event chain for debugging (LLM requests and tool calls)
    #[instrument(skip(self))]
    pub async fn get_event_chain(&self, execution_id: Uuid) -> Result<EventChain> {
        let events = self.store.get_execution_events(execution_id).await?;

        let llm_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e.event_type, EventType::LlmRequest | EventType::LlmResponse))
            .cloned()
            .collect();

        let tool_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e.event_type, EventType::ToolCall | EventType::ToolResult))
            .cloned()
            .collect();

        let error_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == EventType::Error)
            .cloned()
            .collect();

        Ok(EventChain {
            execution_id,
            llm_events,
            tool_events,
            error_events,
        })
    }
}
