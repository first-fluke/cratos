//! Cost Reporting
//!
//! This module contains types for cost reports and savings analysis.

use super::record::UsageStats;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::record::{ModelStats, UsageRecord};

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
// Savings Calculation Constants
// ============================================================================

/// Minimum cost threshold for savings recommendation (USD)
pub const MIN_SAVINGS_THRESHOLD: f64 = 0.01;

/// Assumed percentage of expensive requests that could use cheaper models
pub const EXPENSIVE_MODEL_CANDIDATE_RATIO: f64 = 0.3;

/// Assumed percentage of Opus requests that could use Sonnet
pub const OPUS_SONNET_CANDIDATE_RATIO: f64 = 0.5;

// ============================================================================
// Savings Calculation Functions
// ============================================================================

use super::pricing::{
    CLAUDE_OPUS_INPUT_COST, CLAUDE_OPUS_OUTPUT_COST, CLAUDE_SONNET45_INPUT_COST,
    CLAUDE_SONNET45_OUTPUT_COST, CLAUDE_SONNET_INPUT_COST, CLAUDE_SONNET_OUTPUT_COST,
    DEEPSEEK_CHAT_INPUT_COST, DEEPSEEK_CHAT_OUTPUT_COST, DEEPSEEK_DISTILL_INPUT_COST,
    DEEPSEEK_DISTILL_OUTPUT_COST, GEMINI_FLASH_INPUT_COST, GEMINI_FLASH_OUTPUT_COST,
    GPT4O_INPUT_COST, GPT4O_OUTPUT_COST, GPT5_INPUT_COST, GPT5_NANO_INPUT_COST,
    GPT5_NANO_OUTPUT_COST, GPT5_OUTPUT_COST,
};

/// Calculate potential savings by replacing expensive models with cheaper alternatives
pub fn calculate_gpt_savings(model: &str, model_stats: &ModelStats) -> Option<(f64, String)> {
    // GPT-5 → GPT-5-nano
    if model.contains("gpt-5") && !model.contains("nano") {
        let nano_cost_per_million = GPT5_NANO_INPUT_COST + GPT5_NANO_OUTPUT_COST;
        let current_cost_per_million = GPT5_INPUT_COST + GPT5_OUTPUT_COST;
        let tokens = model_stats.input_tokens + model_stats.output_tokens;
        let current_cost = (tokens as f64 / 1_000_000.0) * current_cost_per_million;
        let nano_cost =
            (tokens as f64 / 1_000_000.0) * nano_cost_per_million * EXPENSIVE_MODEL_CANDIDATE_RATIO;
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
pub fn calculate_opus_savings(model: &str, model_stats: &ModelStats) -> Option<(f64, String)> {
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
pub fn calculate_deepseek_savings(model: &str, model_stats: &ModelStats) -> Option<(f64, String)> {
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
    let deepseek_cost =
        (tokens as f64 / 1_000_000.0) * deepseek_cost_per_million * EXPENSIVE_MODEL_CANDIDATE_RATIO;
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

/// Calculate savings potential from usage stats
pub fn calculate_savings_potential(stats: &UsageStats) -> Option<SavingsPotential> {
    let mut potential_savings = 0.0;
    let mut recommendations = Vec::new();

    for (model, model_stats) in &stats.by_model {
        // Check for GPT savings opportunities
        if let Some((savings, recommendation)) = calculate_gpt_savings(model, model_stats) {
            potential_savings += savings;
            recommendations.push(recommendation);
        }

        // Check for Claude Opus → Sonnet savings
        if let Some((savings, recommendation)) = calculate_opus_savings(model, model_stats) {
            potential_savings += savings;
            recommendations.push(recommendation);
        }

        // Check for premium model → DeepSeek savings
        if let Some((savings, recommendation)) = calculate_deepseek_savings(model, model_stats) {
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
