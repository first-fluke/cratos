//! Model Pricing - LLM cost information
//!
//! This module contains pricing information for various LLM models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Constants
// ============================================================================

/// Default cost per 1M input tokens (USD) for unknown models
pub const DEFAULT_INPUT_COST_PER_MILLION: f64 = 5.0;

/// Default cost per 1M output tokens (USD) for unknown models
pub const DEFAULT_OUTPUT_COST_PER_MILLION: f64 = 15.0;

// ============================================================================
// 2026 Model Pricing Constants (per 1M tokens, USD)
// ============================================================================

// DeepSeek (Ultra-low-cost leader)
/// DeepSeek R1 Distill input cost per 1M tokens
pub const DEEPSEEK_DISTILL_INPUT_COST: f64 = 0.03;
/// DeepSeek R1 Distill output cost per 1M tokens
pub const DEEPSEEK_DISTILL_OUTPUT_COST: f64 = 0.09;
/// DeepSeek Chat (V3) input cost per 1M tokens
pub const DEEPSEEK_CHAT_INPUT_COST: f64 = 0.14;
/// DeepSeek Chat (V3) output cost per 1M tokens
pub const DEEPSEEK_CHAT_OUTPUT_COST: f64 = 0.28;
/// DeepSeek Reasoner (R1) input cost per 1M tokens
pub const DEEPSEEK_REASONER_INPUT_COST: f64 = 0.55;
/// DeepSeek Reasoner (R1) output cost per 1M tokens
pub const DEEPSEEK_REASONER_OUTPUT_COST: f64 = 2.19;

// OpenAI GPT-5 family (Aug 2025~)
/// GPT-5 nano input cost per 1M tokens
pub const GPT5_NANO_INPUT_COST: f64 = 0.05;
/// GPT-5 nano output cost per 1M tokens
pub const GPT5_NANO_OUTPUT_COST: f64 = 0.40;
/// GPT-5 input cost per 1M tokens
pub const GPT5_INPUT_COST: f64 = 1.25;
/// GPT-5 output cost per 1M tokens
pub const GPT5_OUTPUT_COST: f64 = 10.00;

// OpenAI GPT-4o family (legacy)
/// GPT-4o-mini input cost per 1M tokens
pub const GPT4O_MINI_INPUT_COST: f64 = 0.15;
/// GPT-4o-mini output cost per 1M tokens
pub const GPT4O_MINI_OUTPUT_COST: f64 = 0.60;
/// GPT-4o input cost per 1M tokens
pub const GPT4O_INPUT_COST: f64 = 2.50;
/// GPT-4o output cost per 1M tokens
pub const GPT4O_OUTPUT_COST: f64 = 10.00;

// Anthropic Claude 4.5 family (latest, 67% cheaper than Claude 4!)
/// Claude Haiku 4.5 input cost per 1M tokens
pub const CLAUDE_HAIKU45_INPUT_COST: f64 = 1.00;
/// Claude Haiku 4.5 output cost per 1M tokens
pub const CLAUDE_HAIKU45_OUTPUT_COST: f64 = 5.00;
/// Claude Sonnet 4.5 input cost per 1M tokens
pub const CLAUDE_SONNET45_INPUT_COST: f64 = 3.00;
/// Claude Sonnet 4.5 output cost per 1M tokens
pub const CLAUDE_SONNET45_OUTPUT_COST: f64 = 15.00;
/// Claude Opus 4.5 input cost per 1M tokens
pub const CLAUDE_OPUS45_INPUT_COST: f64 = 5.00;
/// Claude Opus 4.5 output cost per 1M tokens
pub const CLAUDE_OPUS45_OUTPUT_COST: f64 = 25.00;

// Anthropic Claude 4 family (legacy)
/// Claude Sonnet 4 input cost per 1M tokens
pub const CLAUDE_SONNET_INPUT_COST: f64 = 3.00;
/// Claude Sonnet 4 output cost per 1M tokens
pub const CLAUDE_SONNET_OUTPUT_COST: f64 = 15.00;
/// Claude Opus 4 input cost per 1M tokens (legacy, expensive!)
pub const CLAUDE_OPUS_INPUT_COST: f64 = 15.00;
/// Claude Opus 4 output cost per 1M tokens
pub const CLAUDE_OPUS_OUTPUT_COST: f64 = 75.00;
/// Claude 3.5 Haiku input cost per 1M tokens
pub const CLAUDE_HAIKU_INPUT_COST: f64 = 0.25;
/// Claude 3.5 Haiku output cost per 1M tokens
pub const CLAUDE_HAIKU_OUTPUT_COST: f64 = 1.25;

// Google Gemini 2.5 family
/// Gemini 2.5 Flash input cost per 1M tokens
pub const GEMINI_FLASH_INPUT_COST: f64 = 0.075;
/// Gemini 2.5 Flash output cost per 1M tokens
pub const GEMINI_FLASH_OUTPUT_COST: f64 = 0.60;
/// Gemini 2.5 Pro input cost per 1M tokens
pub const GEMINI_PRO_INPUT_COST: f64 = 1.25;
/// Gemini 2.5 Pro output cost per 1M tokens
pub const GEMINI_PRO_OUTPUT_COST: f64 = 15.00;

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
#[must_use]
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
