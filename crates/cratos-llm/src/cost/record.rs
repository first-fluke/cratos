//! Usage Records and Statistics
//!
//! This module contains types for tracking LLM usage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
