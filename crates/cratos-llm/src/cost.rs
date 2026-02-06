//! Cost Tracking - LLM usage and cost reporting
//!
//! This module provides cost estimation and tracking for LLM API calls,
//! enabling budget management and cost optimization.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// Constants
// ============================================================================

/// Default cost per 1M input tokens (USD) for unknown models
const DEFAULT_INPUT_COST_PER_MILLION: f64 = 5.0;

/// Default cost per 1M output tokens (USD) for unknown models
const DEFAULT_OUTPUT_COST_PER_MILLION: f64 = 15.0;

/// Maximum records to keep in memory by default
const DEFAULT_MAX_RECORDS: usize = 10_000;

/// Minimum cost threshold for savings recommendation (USD)
const MIN_SAVINGS_THRESHOLD: f64 = 0.01;

/// Assumed percentage of expensive requests that could use cheaper models
const EXPENSIVE_MODEL_CANDIDATE_RATIO: f64 = 0.3;

/// Assumed percentage of Opus requests that could use Sonnet
const OPUS_SONNET_CANDIDATE_RATIO: f64 = 0.5;

// ============================================================================
// 2026 Model Pricing Constants (per 1M tokens, USD)
// ============================================================================

// DeepSeek (Ultra-low-cost leader)
/// DeepSeek R1 Distill input cost per 1M tokens
const DEEPSEEK_DISTILL_INPUT_COST: f64 = 0.03;
/// DeepSeek R1 Distill output cost per 1M tokens
const DEEPSEEK_DISTILL_OUTPUT_COST: f64 = 0.09;
/// DeepSeek Chat (V3) input cost per 1M tokens
const DEEPSEEK_CHAT_INPUT_COST: f64 = 0.14;
/// DeepSeek Chat (V3) output cost per 1M tokens
const DEEPSEEK_CHAT_OUTPUT_COST: f64 = 0.28;
/// DeepSeek Reasoner (R1) input cost per 1M tokens
const DEEPSEEK_REASONER_INPUT_COST: f64 = 0.55;
/// DeepSeek Reasoner (R1) output cost per 1M tokens
const DEEPSEEK_REASONER_OUTPUT_COST: f64 = 2.19;

// OpenAI GPT-5 family (Aug 2025~)
/// GPT-5 nano input cost per 1M tokens
const GPT5_NANO_INPUT_COST: f64 = 0.05;
/// GPT-5 nano output cost per 1M tokens
const GPT5_NANO_OUTPUT_COST: f64 = 0.40;
/// GPT-5 input cost per 1M tokens
const GPT5_INPUT_COST: f64 = 1.25;
/// GPT-5 output cost per 1M tokens
const GPT5_OUTPUT_COST: f64 = 10.00;

// OpenAI GPT-4o family (legacy)
/// GPT-4o-mini input cost per 1M tokens
const GPT4O_MINI_INPUT_COST: f64 = 0.15;
/// GPT-4o-mini output cost per 1M tokens
const GPT4O_MINI_OUTPUT_COST: f64 = 0.60;
/// GPT-4o input cost per 1M tokens
const GPT4O_INPUT_COST: f64 = 2.50;
/// GPT-4o output cost per 1M tokens
const GPT4O_OUTPUT_COST: f64 = 10.00;

// Anthropic Claude 4.5 family (latest, 67% cheaper than Claude 4!)
/// Claude Haiku 4.5 input cost per 1M tokens
const CLAUDE_HAIKU45_INPUT_COST: f64 = 1.00;
/// Claude Haiku 4.5 output cost per 1M tokens
const CLAUDE_HAIKU45_OUTPUT_COST: f64 = 5.00;
/// Claude Sonnet 4.5 input cost per 1M tokens
const CLAUDE_SONNET45_INPUT_COST: f64 = 3.00;
/// Claude Sonnet 4.5 output cost per 1M tokens
const CLAUDE_SONNET45_OUTPUT_COST: f64 = 15.00;
/// Claude Opus 4.5 input cost per 1M tokens
const CLAUDE_OPUS45_INPUT_COST: f64 = 5.00;
/// Claude Opus 4.5 output cost per 1M tokens
const CLAUDE_OPUS45_OUTPUT_COST: f64 = 25.00;

// Anthropic Claude 4 family (legacy)
/// Claude Sonnet 4 input cost per 1M tokens
const CLAUDE_SONNET_INPUT_COST: f64 = 3.00;
/// Claude Sonnet 4 output cost per 1M tokens
const CLAUDE_SONNET_OUTPUT_COST: f64 = 15.00;
/// Claude Opus 4 input cost per 1M tokens (legacy, expensive!)
const CLAUDE_OPUS_INPUT_COST: f64 = 15.00;
/// Claude Opus 4 output cost per 1M tokens
const CLAUDE_OPUS_OUTPUT_COST: f64 = 75.00;
/// Claude 3.5 Haiku input cost per 1M tokens
const CLAUDE_HAIKU_INPUT_COST: f64 = 0.25;
/// Claude 3.5 Haiku output cost per 1M tokens
const CLAUDE_HAIKU_OUTPUT_COST: f64 = 1.25;

// Google Gemini 2.5 family
/// Gemini 2.5 Flash input cost per 1M tokens
const GEMINI_FLASH_INPUT_COST: f64 = 0.075;
/// Gemini 2.5 Flash output cost per 1M tokens
const GEMINI_FLASH_OUTPUT_COST: f64 = 0.60;
/// Gemini 2.5 Pro input cost per 1M tokens
const GEMINI_PRO_INPUT_COST: f64 = 1.25;
/// Gemini 2.5 Pro output cost per 1M tokens
const GEMINI_PRO_OUTPUT_COST: f64 = 15.00;

// ============================================================================
// Cost Models
// ============================================================================

/// Pricing information for a model (per 1M tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Model name
    pub model: String,
    /// Provider name
    pub provider: String,
    /// Cost per 1M input tokens (USD)
    pub input_cost_per_million: f64,
    /// Cost per 1M output tokens (USD)
    pub output_cost_per_million: f64,
    /// Context window size
    pub context_window: u32,
    /// Last updated
    pub updated_at: DateTime<Utc>,
}

impl ModelPricing {
    /// Calculate cost for given token counts
    #[must_use]
    pub fn calculate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input_cost_per_million;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.output_cost_per_million;
        input_cost + output_cost
    }
}

/// Default pricing for common models (2026 pricing)
pub fn default_pricing() -> HashMap<String, ModelPricing> {
    let now = Utc::now();
    let mut pricing = HashMap::new();

    // ========================================================================
    // DeepSeek Models (Ultra-low-cost leader)
    // ========================================================================
    pricing.insert(
        "deepseek-r1-distill-llama-70b".to_string(),
        ModelPricing {
            model: "deepseek-r1-distill-llama-70b".to_string(),
            provider: "deepseek".to_string(),
            input_cost_per_million: DEEPSEEK_DISTILL_INPUT_COST,
            output_cost_per_million: DEEPSEEK_DISTILL_OUTPUT_COST,
            context_window: 64_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "deepseek-chat".to_string(),
        ModelPricing {
            model: "deepseek-chat".to_string(),
            provider: "deepseek".to_string(),
            input_cost_per_million: DEEPSEEK_CHAT_INPUT_COST,
            output_cost_per_million: DEEPSEEK_CHAT_OUTPUT_COST,
            context_window: 64_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "deepseek-reasoner".to_string(),
        ModelPricing {
            model: "deepseek-reasoner".to_string(),
            provider: "deepseek".to_string(),
            input_cost_per_million: DEEPSEEK_REASONER_INPUT_COST,
            output_cost_per_million: DEEPSEEK_REASONER_OUTPUT_COST,
            context_window: 64_000,
            updated_at: now,
        },
    );

    // ========================================================================
    // OpenAI GPT-5 Family (Aug 2025~)
    // ========================================================================
    pricing.insert(
        "gpt-5-nano".to_string(),
        ModelPricing {
            model: "gpt-5-nano".to_string(),
            provider: "openai".to_string(),
            input_cost_per_million: GPT5_NANO_INPUT_COST,
            output_cost_per_million: GPT5_NANO_OUTPUT_COST,
            context_window: 32_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "gpt-5".to_string(),
        ModelPricing {
            model: "gpt-5".to_string(),
            provider: "openai".to_string(),
            input_cost_per_million: GPT5_INPUT_COST,
            output_cost_per_million: GPT5_OUTPUT_COST,
            context_window: 400_000,
            updated_at: now,
        },
    );

    // ========================================================================
    // OpenAI GPT-4o Family (legacy)
    // ========================================================================
    pricing.insert(
        "gpt-4o".to_string(),
        ModelPricing {
            model: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            input_cost_per_million: GPT4O_INPUT_COST,
            output_cost_per_million: GPT4O_OUTPUT_COST,
            context_window: 128_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "gpt-4o-mini".to_string(),
        ModelPricing {
            model: "gpt-4o-mini".to_string(),
            provider: "openai".to_string(),
            input_cost_per_million: GPT4O_MINI_INPUT_COST,
            output_cost_per_million: GPT4O_MINI_OUTPUT_COST,
            context_window: 128_000,
            updated_at: now,
        },
    );

    // ========================================================================
    // Anthropic Claude 4.5 Family (latest — 67% cheaper than Claude 4!)
    // ========================================================================
    pricing.insert(
        "claude-opus-4-5-20250514".to_string(),
        ModelPricing {
            model: "claude-opus-4-5-20250514".to_string(),
            provider: "anthropic".to_string(),
            input_cost_per_million: CLAUDE_OPUS45_INPUT_COST,
            output_cost_per_million: CLAUDE_OPUS45_OUTPUT_COST,
            context_window: 200_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "claude-sonnet-4-5-20250929".to_string(),
        ModelPricing {
            model: "claude-sonnet-4-5-20250929".to_string(),
            provider: "anthropic".to_string(),
            input_cost_per_million: CLAUDE_SONNET45_INPUT_COST,
            output_cost_per_million: CLAUDE_SONNET45_OUTPUT_COST,
            context_window: 200_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "claude-haiku-4-5-20251001".to_string(),
        ModelPricing {
            model: "claude-haiku-4-5-20251001".to_string(),
            provider: "anthropic".to_string(),
            input_cost_per_million: CLAUDE_HAIKU45_INPUT_COST,
            output_cost_per_million: CLAUDE_HAIKU45_OUTPUT_COST,
            context_window: 200_000,
            updated_at: now,
        },
    );

    // ========================================================================
    // Anthropic Claude 4 Family (legacy)
    // ========================================================================
    pricing.insert(
        "claude-opus-4-20250514".to_string(),
        ModelPricing {
            model: "claude-opus-4-20250514".to_string(),
            provider: "anthropic".to_string(),
            input_cost_per_million: CLAUDE_OPUS_INPUT_COST,
            output_cost_per_million: CLAUDE_OPUS_OUTPUT_COST,
            context_window: 200_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "claude-sonnet-4-20250514".to_string(),
        ModelPricing {
            model: "claude-sonnet-4-20250514".to_string(),
            provider: "anthropic".to_string(),
            input_cost_per_million: CLAUDE_SONNET_INPUT_COST,
            output_cost_per_million: CLAUDE_SONNET_OUTPUT_COST,
            context_window: 200_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "claude-3-5-haiku-20241022".to_string(),
        ModelPricing {
            model: "claude-3-5-haiku-20241022".to_string(),
            provider: "anthropic".to_string(),
            input_cost_per_million: CLAUDE_HAIKU_INPUT_COST,
            output_cost_per_million: CLAUDE_HAIKU_OUTPUT_COST,
            context_window: 200_000,
            updated_at: now,
        },
    );

    // Legacy Claude 3.5 Sonnet (still available)
    pricing.insert(
        "claude-3-5-sonnet-20241022".to_string(),
        ModelPricing {
            model: "claude-3-5-sonnet-20241022".to_string(),
            provider: "anthropic".to_string(),
            input_cost_per_million: 3.00,
            output_cost_per_million: 15.00,
            context_window: 200_000,
            updated_at: now,
        },
    );

    // Legacy Claude 3 Opus
    pricing.insert(
        "claude-3-opus-20240229".to_string(),
        ModelPricing {
            model: "claude-3-opus-20240229".to_string(),
            provider: "anthropic".to_string(),
            input_cost_per_million: 15.00,
            output_cost_per_million: 75.00,
            context_window: 200_000,
            updated_at: now,
        },
    );

    // ========================================================================
    // Google Gemini 2.5 Family
    // ========================================================================
    pricing.insert(
        "gemini-2.5-flash".to_string(),
        ModelPricing {
            model: "gemini-2.5-flash".to_string(),
            provider: "gemini".to_string(),
            input_cost_per_million: GEMINI_FLASH_INPUT_COST,
            output_cost_per_million: GEMINI_FLASH_OUTPUT_COST,
            context_window: 1_000_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "gemini-2.5-pro".to_string(),
        ModelPricing {
            model: "gemini-2.5-pro".to_string(),
            provider: "gemini".to_string(),
            input_cost_per_million: GEMINI_PRO_INPUT_COST,
            output_cost_per_million: GEMINI_PRO_OUTPUT_COST,
            context_window: 1_000_000,
            updated_at: now,
        },
    );

    // Legacy Gemini 1.5 models
    pricing.insert(
        "gemini-1.5-pro".to_string(),
        ModelPricing {
            model: "gemini-1.5-pro".to_string(),
            provider: "gemini".to_string(),
            input_cost_per_million: 1.25,
            output_cost_per_million: 5.00,
            context_window: 2_000_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "gemini-1.5-flash".to_string(),
        ModelPricing {
            model: "gemini-1.5-flash".to_string(),
            provider: "gemini".to_string(),
            input_cost_per_million: 0.075,
            output_cost_per_million: 0.30,
            context_window: 1_000_000,
            updated_at: now,
        },
    );

    // ========================================================================
    // Groq (low-cost, free tier available with rate limits)
    // ========================================================================
    pricing.insert(
        "llama-3.1-8b-instant".to_string(),
        ModelPricing {
            model: "llama-3.1-8b-instant".to_string(),
            provider: "groq".to_string(),
            input_cost_per_million: 0.05,
            output_cost_per_million: 0.08,
            context_window: 128_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "openai/gpt-oss-20b".to_string(),
        ModelPricing {
            model: "openai/gpt-oss-20b".to_string(),
            provider: "groq".to_string(),
            input_cost_per_million: 0.075,
            output_cost_per_million: 0.30,
            context_window: 128_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "openai/gpt-oss-120b".to_string(),
        ModelPricing {
            model: "openai/gpt-oss-120b".to_string(),
            provider: "groq".to_string(),
            input_cost_per_million: 0.15,
            output_cost_per_million: 0.60,
            context_window: 128_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "llama-3.3-70b-versatile".to_string(),
        ModelPricing {
            model: "llama-3.3-70b-versatile".to_string(),
            provider: "groq".to_string(),
            input_cost_per_million: 0.59,
            output_cost_per_million: 0.79,
            context_window: 128_000,
            updated_at: now,
        },
    );

    // ========================================================================
    // Local models via Ollama (FREE)
    // ========================================================================
    pricing.insert(
        "qwen2.5:7b".to_string(),
        ModelPricing {
            model: "qwen2.5:7b".to_string(),
            provider: "ollama".to_string(),
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
            context_window: 128_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "mistral".to_string(),
        ModelPricing {
            model: "mistral".to_string(),
            provider: "ollama".to_string(),
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
            context_window: 32_000,
            updated_at: now,
        },
    );

    // ========================================================================
    // Qwen 3 Family
    // ========================================================================
    pricing.insert(
        "qwen3-8b".to_string(),
        ModelPricing {
            model: "qwen3-8b".to_string(),
            provider: "qwen".to_string(),
            input_cost_per_million: 0.06,
            output_cost_per_million: 0.09,
            context_window: 128_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "qwen3-32b".to_string(),
        ModelPricing {
            model: "qwen3-32b".to_string(),
            provider: "qwen".to_string(),
            input_cost_per_million: 0.20,
            output_cost_per_million: 0.30,
            context_window: 128_000,
            updated_at: now,
        },
    );

    // ========================================================================
    // GLM-4 Family
    // ========================================================================
    pricing.insert(
        "glm-4-9b".to_string(),
        ModelPricing {
            model: "glm-4-9b".to_string(),
            provider: "glm".to_string(),
            input_cost_per_million: 0.086,
            output_cost_per_million: 0.086,
            context_window: 128_000,
            updated_at: now,
        },
    );

    pricing.insert(
        "glm-4-flash".to_string(),
        ModelPricing {
            model: "glm-4-flash".to_string(),
            provider: "glm".to_string(),
            input_cost_per_million: 0.01,
            output_cost_per_million: 0.01,
            context_window: 128_000,
            updated_at: now,
        },
    );

    pricing
}

// ============================================================================
// Usage Tracking
// ============================================================================

/// A single usage record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// Record ID
    pub id: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Execution ID (if available)
    pub execution_id: Option<String>,
    /// Provider name
    pub provider: String,
    /// Model name
    pub model: String,
    /// Input tokens
    pub input_tokens: u32,
    /// Output tokens
    pub output_tokens: u32,
    /// Estimated cost (USD)
    pub estimated_cost: f64,
    /// Latency in milliseconds
    pub latency_ms: u64,
    /// Was the request successful?
    pub success: bool,
}

/// Aggregated usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    /// Total input tokens
    pub total_input_tokens: u64,
    /// Total output tokens
    pub total_output_tokens: u64,
    /// Total estimated cost (USD)
    pub total_cost: f64,
    /// Total requests
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Average latency (ms)
    pub avg_latency_ms: f64,
    /// Usage by provider
    pub by_provider: HashMap<String, ProviderStats>,
    /// Usage by model
    pub by_model: HashMap<String, ModelStats>,
}

/// Per-provider statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderStats {
    /// Provider name
    pub provider: String,
    /// Total tokens
    pub total_tokens: u64,
    /// Total cost
    pub total_cost: f64,
    /// Request count
    pub request_count: u64,
}

/// Per-model statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelStats {
    /// Model name
    pub model: String,
    /// Total input tokens
    pub input_tokens: u64,
    /// Total output tokens
    pub output_tokens: u64,
    /// Total cost
    pub total_cost: f64,
    /// Request count
    pub request_count: u64,
}

// ============================================================================
// Cost Tracker
// ============================================================================

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
        let savings_potential = self.calculate_savings_potential(&stats);

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

    fn calculate_savings_potential(&self, stats: &UsageStats) -> Option<SavingsPotential> {
        let mut potential_savings = 0.0;
        let mut recommendations = Vec::new();

        for (model, model_stats) in &stats.by_model {
            // Check for GPT savings opportunities
            if let Some((savings, recommendation)) = self.calculate_gpt_savings(model, model_stats)
            {
                potential_savings += savings;
                recommendations.push(recommendation);
            }

            // Check for Claude Opus → Sonnet savings
            if let Some((savings, recommendation)) = self.calculate_opus_savings(model, model_stats)
            {
                potential_savings += savings;
                recommendations.push(recommendation);
            }

            // Check for premium model → DeepSeek savings
            if let Some((savings, recommendation)) =
                self.calculate_deepseek_savings(model, model_stats)
            {
                potential_savings += savings;
                recommendations.push(recommendation);
            }
        }

        if potential_savings > MIN_SAVINGS_THRESHOLD {
            Some(SavingsPotential {
                estimated_savings: potential_savings,
                recommendations,
            })
        } else {
            None
        }
    }

    /// Calculate potential savings by replacing expensive models with cheaper alternatives
    ///
    /// Examples:
    /// - GPT-5 → GPT-5-nano for simple tasks
    /// - GPT-4o → DeepSeek for routine tasks
    fn calculate_gpt_savings(
        &self,
        model: &str,
        model_stats: &ModelStats,
    ) -> Option<(f64, String)> {
        // GPT-5 → GPT-5-nano
        if model.contains("gpt-5") && !model.contains("nano") {
            let nano_cost_per_million = GPT5_NANO_INPUT_COST + GPT5_NANO_OUTPUT_COST;
            let current_cost_per_million = GPT5_INPUT_COST + GPT5_OUTPUT_COST;
            let tokens = model_stats.input_tokens + model_stats.output_tokens;
            let current_cost = (tokens as f64 / 1_000_000.0) * current_cost_per_million;
            let nano_cost = (tokens as f64 / 1_000_000.0)
                * nano_cost_per_million
                * EXPENSIVE_MODEL_CANDIDATE_RATIO;
            let savings = current_cost * EXPENSIVE_MODEL_CANDIDATE_RATIO - nano_cost;

            if savings > MIN_SAVINGS_THRESHOLD {
                return Some((
                    savings,
                    format!(
                        "Consider using gpt-5-nano for simple tasks (est. ${:.2} savings)",
                        savings
                    ),
                ));
            }
        }

        // GPT-4o → DeepSeek (ultra-low-cost alternative)
        if model.contains("gpt-4o") && !model.contains("mini") {
            let deepseek_cost_per_million = DEEPSEEK_CHAT_INPUT_COST + DEEPSEEK_CHAT_OUTPUT_COST;
            let current_cost_per_million = GPT4O_INPUT_COST + GPT4O_OUTPUT_COST;
            let tokens = model_stats.input_tokens + model_stats.output_tokens;
            let current_cost = (tokens as f64 / 1_000_000.0) * current_cost_per_million;
            let deepseek_cost = (tokens as f64 / 1_000_000.0)
                * deepseek_cost_per_million
                * EXPENSIVE_MODEL_CANDIDATE_RATIO;
            let savings = current_cost * EXPENSIVE_MODEL_CANDIDATE_RATIO - deepseek_cost;

            if savings > MIN_SAVINGS_THRESHOLD {
                return Some((
                    savings,
                    format!(
                        "Consider using deepseek-chat for simple tasks (est. ${:.2} savings)",
                        savings
                    ),
                ));
            }
        }

        None
    }

    /// Calculate potential savings by replacing Claude Opus with Sonnet for routine tasks
    fn calculate_opus_savings(
        &self,
        model: &str,
        model_stats: &ModelStats,
    ) -> Option<(f64, String)> {
        if !model.contains("opus") {
            return None;
        }

        let sonnet_cost_per_million = CLAUDE_SONNET_INPUT_COST + CLAUDE_SONNET_OUTPUT_COST;
        let current_cost_per_million = CLAUDE_OPUS_INPUT_COST + CLAUDE_OPUS_OUTPUT_COST;
        let tokens = model_stats.input_tokens + model_stats.output_tokens;
        let current_cost = (tokens as f64 / 1_000_000.0) * current_cost_per_million;
        let sonnet_cost =
            (tokens as f64 / 1_000_000.0) * sonnet_cost_per_million * OPUS_SONNET_CANDIDATE_RATIO;
        let savings = current_cost * OPUS_SONNET_CANDIDATE_RATIO - sonnet_cost;

        if savings > MIN_SAVINGS_THRESHOLD {
            Some((
                savings,
                format!(
                    "Consider using Claude Sonnet for routine tasks (est. ${:.2} savings)",
                    savings
                ),
            ))
        } else {
            None
        }
    }

    /// Calculate potential savings by switching to DeepSeek for non-premium tasks
    ///
    /// DeepSeek R1 Distill is the cheapest option at $0.03/$0.09 per 1M tokens.
    fn calculate_deepseek_savings(
        &self,
        model: &str,
        model_stats: &ModelStats,
    ) -> Option<(f64, String)> {
        // Skip if already using DeepSeek or free models
        if model.contains("deepseek")
            || model.contains("llama")
            || model.contains("groq")
            || model.contains("ollama")
        {
            return None;
        }

        // Skip premium models (Opus) - they're used for quality reasons
        if model.contains("opus") || model.contains("ultra") {
            return None;
        }

        // Calculate potential savings by switching to DeepSeek R1 Distill
        let deepseek_cost_per_million = DEEPSEEK_DISTILL_INPUT_COST + DEEPSEEK_DISTILL_OUTPUT_COST;

        // Estimate current cost based on model family
        let current_cost_per_million = if model.contains("gemini") {
            GEMINI_FLASH_INPUT_COST + GEMINI_FLASH_OUTPUT_COST
        } else if model.contains("claude") || model.contains("sonnet") {
            CLAUDE_SONNET45_INPUT_COST + CLAUDE_SONNET45_OUTPUT_COST
        } else if model.contains("gpt-5") {
            GPT5_INPUT_COST + GPT5_OUTPUT_COST
        } else if model.contains("gpt") {
            GPT4O_INPUT_COST + GPT4O_OUTPUT_COST
        } else {
            5.0 // Default assumption
        };

        let tokens = model_stats.input_tokens + model_stats.output_tokens;
        let current_cost = (tokens as f64 / 1_000_000.0) * current_cost_per_million;
        let deepseek_cost = (tokens as f64 / 1_000_000.0)
            * deepseek_cost_per_million
            * EXPENSIVE_MODEL_CANDIDATE_RATIO;
        let savings = current_cost * EXPENSIVE_MODEL_CANDIDATE_RATIO - deepseek_cost;

        if savings > MIN_SAVINGS_THRESHOLD {
            Some((
                savings,
                format!(
                    "Consider deepseek-r1-distill for trivial tasks (est. ${:.2} savings)",
                    savings
                ),
            ))
        } else {
            None
        }
    }

    /// Format report as text
    #[must_use]
    pub fn format_report(report: &CostReport) -> String {
        let mut output = String::new();

        output.push_str("📊 **LLM Cost Report**\n\n");
        output.push_str(&format!(
            "Generated: {}\n",
            report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        if let Some(start) = report.period_start {
            output.push_str(&format!("Period: {} to now\n", start.format("%Y-%m-%d")));
        }

        output.push_str("\n**Summary:**\n");
        output.push_str(&format!(
            "• Total Requests: {} ({} success, {} failed)\n",
            report.stats.total_requests,
            report.stats.successful_requests,
            report.stats.failed_requests
        ));
        output.push_str(&format!(
            "• Total Tokens: {} input, {} output\n",
            report.stats.total_input_tokens, report.stats.total_output_tokens
        ));
        output.push_str(&format!("• Total Cost: ${:.4}\n", report.stats.total_cost));
        output.push_str(&format!(
            "• Avg Latency: {:.0}ms\n",
            report.stats.avg_latency_ms
        ));

        if let Some(model) = &report.most_used_model {
            output.push_str(&format!("• Most Used Model: {}\n", model));
        }

        if let Some(model) = &report.most_expensive_model {
            output.push_str(&format!("• Most Expensive Model: {}\n", model));
        }

        output.push_str("\n**By Provider:**\n");
        for (provider, stats) in &report.stats.by_provider {
            output.push_str(&format!(
                "• {}: {} requests, ${:.4}\n",
                provider, stats.request_count, stats.total_cost
            ));
        }

        output.push_str("\n**By Model:**\n");
        for (model, stats) in &report.stats.by_model {
            output.push_str(&format!(
                "• {}: {} requests, {} tokens, ${:.4}\n",
                model,
                stats.request_count,
                stats.input_tokens + stats.output_tokens,
                stats.total_cost
            ));
        }

        if let Some(savings) = &report.savings_potential {
            output.push_str(&format!(
                "\n💡 **Potential Savings:** ${:.2}\n",
                savings.estimated_savings
            ));
            for rec in &savings.recommendations {
                output.push_str(&format!("• {}\n", rec));
            }
        }

        output
    }
}

/// Cost report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    /// When report was generated
    pub generated_at: DateTime<Utc>,
    /// Start of reporting period
    pub period_start: Option<DateTime<Utc>>,
    /// Usage statistics
    pub stats: UsageStats,
    /// Recent records
    pub recent_records: Vec<UsageRecord>,
    /// Most expensive model
    pub most_expensive_model: Option<String>,
    /// Most used model
    pub most_used_model: Option<String>,
    /// Savings potential
    pub savings_potential: Option<SavingsPotential>,
}

/// Potential cost savings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsPotential {
    /// Estimated savings in USD
    pub estimated_savings: f64,
    /// Recommendations
    pub recommendations: Vec<String>,
}

// ============================================================================
// Global Tracker
// ============================================================================

lazy_static::lazy_static! {
    /// Global cost tracker instance
    static ref GLOBAL_TRACKER: Arc<CostTracker> = Arc::new(CostTracker::new());
}

/// Get the global cost tracker
#[must_use]
pub fn global_tracker() -> Arc<CostTracker> {
    Arc::clone(&GLOBAL_TRACKER)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_pricing_calculation() {
        let pricing = ModelPricing {
            model: "test-model".to_string(),
            provider: "test".to_string(),
            input_cost_per_million: 10.0,
            output_cost_per_million: 20.0,
            context_window: 128_000,
            updated_at: Utc::now(),
        };

        // 1M tokens each
        let cost = pricing.calculate_cost(1_000_000, 1_000_000);
        assert!((cost - 30.0).abs() < 0.001);

        // 1K tokens each
        let cost = pricing.calculate_cost(1_000, 1_000);
        assert!((cost - 0.03).abs() < 0.001);
    }

    #[test]
    fn test_default_pricing_has_common_models() {
        let pricing = default_pricing();

        // OpenAI GPT-5
        assert!(pricing.contains_key("gpt-5"));
        assert!(pricing.contains_key("gpt-5-nano"));
        // OpenAI GPT-4o (legacy)
        assert!(pricing.contains_key("gpt-4o"));
        assert!(pricing.contains_key("gpt-4o-mini"));

        // Anthropic Claude 4.5
        assert!(pricing.contains_key("claude-opus-4-5-20250514"));
        assert!(pricing.contains_key("claude-sonnet-4-5-20250929"));
        assert!(pricing.contains_key("claude-haiku-4-5-20251001"));
        // Anthropic legacy
        assert!(pricing.contains_key("claude-sonnet-4-20250514"));

        // Gemini
        assert!(pricing.contains_key("gemini-2.5-flash"));
        assert!(pricing.contains_key("gemini-2.5-pro"));

        // Groq
        assert!(pricing.contains_key("llama-3.3-70b-versatile"));
        assert!(pricing.contains_key("openai/gpt-oss-20b"));

        // DeepSeek
        assert!(pricing.contains_key("deepseek-r1-distill-llama-70b"));

        // Free models
        assert!(pricing.contains_key("llama-3.3-70b-versatile"));
        assert!(pricing.contains_key("qwen2.5:7b"));
    }

    #[tokio::test]
    async fn test_cost_tracker_record_and_stats() {
        let tracker = CostTracker::new();

        // Record some usage
        tracker
            .record_usage("openai", "gpt-4o-mini", 1000, 500, 100, true, None)
            .await;
        tracker
            .record_usage(
                "anthropic",
                "claude-3-5-sonnet-20241022",
                2000,
                1000,
                200,
                true,
                None,
            )
            .await;
        tracker
            .record_usage("openai", "gpt-4o", 500, 200, 150, false, None)
            .await;

        let stats = tracker.get_stats(None).await;

        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 2);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.total_input_tokens, 3500);
        assert_eq!(stats.total_output_tokens, 1700);
        assert!(stats.total_cost > 0.0);
    }

    #[tokio::test]
    async fn test_estimate_cost() {
        let tracker = CostTracker::new();

        // GPT-5.2-mini: $0.15/1M input, $0.60/1M output
        let cost = tracker
            .estimate_cost("gpt-4o-mini", 1_000_000, 1_000_000)
            .await;
        assert!((cost - 0.75).abs() < 0.01);

        // Unknown model should use default
        let cost = tracker
            .estimate_cost("unknown-model", 1_000_000, 1_000_000)
            .await;
        assert!(cost > 0.0);
    }

    #[tokio::test]
    async fn test_get_execution_records() {
        let tracker = CostTracker::new();

        tracker
            .record_usage(
                "openai",
                "gpt-4o",
                100,
                50,
                100,
                true,
                Some("exec-1".to_string()),
            )
            .await;
        tracker
            .record_usage(
                "openai",
                "gpt-4o",
                200,
                100,
                100,
                true,
                Some("exec-1".to_string()),
            )
            .await;
        tracker
            .record_usage(
                "openai",
                "gpt-4o",
                150,
                75,
                100,
                true,
                Some("exec-2".to_string()),
            )
            .await;

        let exec1_records = tracker.get_execution_records("exec-1").await;
        assert_eq!(exec1_records.len(), 2);

        let exec2_records = tracker.get_execution_records("exec-2").await;
        assert_eq!(exec2_records.len(), 1);
    }

    #[tokio::test]
    async fn test_generate_report() {
        let tracker = CostTracker::new();

        tracker
            .record_usage("openai", "gpt-4o", 10000, 5000, 100, true, None)
            .await;
        tracker
            .record_usage(
                "anthropic",
                "claude-3-5-sonnet-20241022",
                20000,
                10000,
                200,
                true,
                None,
            )
            .await;

        let report = tracker.generate_report(None).await;

        assert_eq!(report.stats.total_requests, 2);
        assert!(report.stats.total_cost > 0.0);
        assert!(!report.stats.by_provider.is_empty());
        assert!(!report.stats.by_model.is_empty());
    }

    #[tokio::test]
    async fn test_format_report() {
        let tracker = CostTracker::new();

        tracker
            .record_usage("openai", "gpt-4o", 10000, 5000, 100, true, None)
            .await;

        let report = tracker.generate_report(None).await;
        let formatted = CostTracker::format_report(&report);

        assert!(formatted.contains("Cost Report"));
        assert!(formatted.contains("Total Requests"));
        assert!(formatted.contains("openai"));
    }

    #[test]
    fn test_global_tracker() {
        let tracker1 = global_tracker();
        let tracker2 = global_tracker();

        // Should be the same instance
        assert!(Arc::ptr_eq(&tracker1, &tracker2));
    }
}
