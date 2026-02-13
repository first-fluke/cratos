//! Cost Tracking - LLM usage and cost reporting
//!
//! This module provides cost estimation and tracking for LLM API calls,
//! enabling budget management and cost optimization.
//!
//! # Module Structure
//!
//! - `pricing`: Model pricing information and defaults
//! - `record`: Usage records and statistics types
//! - `tracker`: CostTracker implementation
//! - `report`: Cost reports and savings analysis
//! - `global`: Global tracker singleton

mod global;
mod pricing;
mod record;
mod report;
mod tracker;

#[cfg(test)]
mod tests;

// Re-export public types
pub use global::global_tracker;
pub use pricing::{default_pricing, ModelPricing};
pub use record::{ModelStats, ProviderStats, UsageRecord, UsageStats};
pub use report::{CostReport, SavingsPotential};
pub use tracker::CostTracker;
