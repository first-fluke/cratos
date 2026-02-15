//! Cost Tracker - Usage monitoring
//!
//! This module contains the CostTracker for monitoring LLM usage.

use super::pricing::{
    default_pricing, ModelPricing, DEFAULT_INPUT_COST_PER_MILLION, DEFAULT_OUTPUT_COST_PER_MILLION,
};
use super::record::{ModelStats, ProviderStats, UsageRecord, UsageStats};
use super::report::{calculate_savings_potential, CostReport};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// Maximum records to keep in memory by default
const DEFAULT_MAX_RECORDS: usize = 10_000;

/// Cost tracker for monitoring LLM usage
#[derive(Debug)]
pub struct CostTracker {
    /// Pricing information
    pricing: RwLock<HashMap<String, ModelPricing>>,
    /// Usage records
    records: RwLock<Vec<UsageRecord>>,
    /// Record ID counter
    next_id: AtomicU64,
    /// Maximum records to keep in memory
    max_records: usize,
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CostTracker {
    /// Create a new cost tracker with default pricing
    #[must_use]
    pub fn new() -> Self {
        Self {
            pricing: RwLock::new(default_pricing()),
            records: RwLock::new(Vec::new()),
            next_id: AtomicU64::new(1),
            max_records: DEFAULT_MAX_RECORDS,
        }
    }

    /// Create with custom max records
    #[must_use]
    pub fn with_max_records(mut self, max: usize) -> Self {
        self.max_records = max;
        self
    }

    /// Update pricing for a model
    pub async fn update_pricing(&self, model: &str, pricing: ModelPricing) {
        let mut prices = self.pricing.write().await;
        prices.insert(model.to_string(), pricing);
    }

    /// Get pricing for a model
    pub async fn get_pricing(&self, model: &str) -> Option<ModelPricing> {
        let prices = self.pricing.read().await;
        prices.get(model).cloned()
    }

    /// Estimate cost for a request
    pub async fn estimate_cost(&self, model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
        let prices = self.pricing.read().await;
        if let Some(pricing) = prices.get(model) {
            pricing.calculate_cost(input_tokens, output_tokens)
        } else {
            // Default estimate for unknown models
            (input_tokens as f64 / 1_000_000.0) * DEFAULT_INPUT_COST_PER_MILLION
                + (output_tokens as f64 / 1_000_000.0) * DEFAULT_OUTPUT_COST_PER_MILLION
        }
    }

    /// Record a usage event
    #[allow(clippy::too_many_arguments)]
    pub async fn record_usage(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        latency_ms: u64,
        success: bool,
        execution_id: Option<String>,
    ) -> UsageRecord {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let estimated_cost = self.estimate_cost(model, input_tokens, output_tokens).await;

        let record = UsageRecord {
            id,
            timestamp: Utc::now(),
            execution_id,
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens,
            output_tokens,
            estimated_cost,
            latency_ms,
            success,
        };

        let mut records = self.records.write().await;
        records.push(record.clone());

        // Trim old records if needed
        if records.len() > self.max_records {
            let drain_count = records.len() - self.max_records;
            records.drain(0..drain_count);
        }

        record
    }

    /// Get usage statistics for a time range
    pub async fn get_stats(&self, since: Option<DateTime<Utc>>) -> UsageStats {
        let records = self.records.read().await;

        let filtered: Vec<_> = if let Some(since) = since {
            records.iter().filter(|r| r.timestamp >= since).collect()
        } else {
            records.iter().collect()
        };

        let mut stats = UsageStats::default();

        for record in filtered {
            stats.total_input_tokens += record.input_tokens as u64;
            stats.total_output_tokens += record.output_tokens as u64;
            stats.total_cost += record.estimated_cost;
            stats.total_requests += 1;

            if record.success {
                stats.successful_requests += 1;
            } else {
                stats.failed_requests += 1;
            }

            // Per-provider stats
            let provider_stats = stats
                .by_provider
                .entry(record.provider.clone())
                .or_insert_with(|| ProviderStats {
                    provider: record.provider.clone(),
                    ..Default::default()
                });
            provider_stats.total_tokens += (record.input_tokens + record.output_tokens) as u64;
            provider_stats.total_cost += record.estimated_cost;
            provider_stats.request_count += 1;

            // Per-model stats
            let model_stats = stats
                .by_model
                .entry(record.model.clone())
                .or_insert_with(|| ModelStats {
                    model: record.model.clone(),
                    ..Default::default()
                });
            model_stats.input_tokens += record.input_tokens as u64;
            model_stats.output_tokens += record.output_tokens as u64;
            model_stats.total_cost += record.estimated_cost;
            model_stats.request_count += 1;
        }

        // Calculate average latency
        let total_latency: u64 = records.iter().map(|r| r.latency_ms).sum();
        if !records.is_empty() {
            stats.avg_latency_ms = total_latency as f64 / records.len() as f64;
        }

        stats
    }

    /// Non-blocking stats snapshot (for TUI render loop).
    /// Returns `None` if the lock is held by another task.
    pub fn try_get_stats(&self) -> Option<UsageStats> {
        let records = self.records.try_read().ok()?;
        let mut stats = UsageStats::default();

        for record in records.iter() {
            stats.total_input_tokens += record.input_tokens as u64;
            stats.total_output_tokens += record.output_tokens as u64;
            stats.total_cost += record.estimated_cost;
            stats.total_requests += 1;

            if record.success {
                stats.successful_requests += 1;
            } else {
                stats.failed_requests += 1;
            }
        }

        Some(stats)
    }

    /// Get recent records
    pub async fn get_recent_records(&self, limit: usize) -> Vec<UsageRecord> {
        let records = self.records.read().await;
        let start = if records.len() > limit {
            records.len() - limit
        } else {
            0
        };
        records[start..].to_vec()
    }

    /// Get records for a specific execution
    pub async fn get_execution_records(&self, execution_id: &str) -> Vec<UsageRecord> {
        let records = self.records.read().await;
        records
            .iter()
            .filter(|r| r.execution_id.as_deref() == Some(execution_id))
            .cloned()
            .collect()
    }

    /// Generate a cost report
    pub async fn generate_report(&self, since: Option<DateTime<Utc>>) -> CostReport {
        let stats = self.get_stats(since).await;
        let records = self.get_recent_records(100).await;

        // Find most expensive model
        let most_expensive_model = stats
            .by_model
            .iter()
            .max_by(|a, b| {
                a.1.total_cost
                    .partial_cmp(&b.1.total_cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, _)| name.clone());

        // Find most used model
        let most_used_model = stats
            .by_model
            .iter()
            .max_by_key(|(_, s)| s.request_count)
            .map(|(name, _)| name.clone());

        // Calculate cost savings potential
        let savings_potential = calculate_savings_potential(&stats);

        CostReport {
            generated_at: Utc::now(),
            period_start: since,
            stats,
            recent_records: records,
            most_expensive_model,
            most_used_model,
            savings_potential,
        }
    }

    /// Format report as text
    #[must_use]
    pub fn format_report(report: &CostReport) -> String {
        super::report::format_report(report)
    }
}
