//! Core types for LLM routing
//!
//! This module contains TaskType and ModelTier enums for intelligent model selection.

use crate::token::TokenBudget;
use serde::{Deserialize, Serialize};

// ============================================================================
// Task Type
// ============================================================================

/// Task type for intelligent model routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Simple classification or intent detection
    Classification,
    /// Complex planning and reasoning
    Planning,
    /// Code generation and modification
    CodeGeneration,
    /// Text summarization
    Summarization,
    /// General conversation
    Conversation,
    /// Data extraction and parsing
    Extraction,
    /// Code review and analysis
    CodeReview,
    /// Translation
    Translation,
}

impl TaskType {
    /// Get the recommended model tier for this task type
    ///
    /// Tier selection optimizes cost while maintaining quality:
    /// - UltraBudget: Trivial tasks (classification, extraction, translation)
    /// - Fast: Simple tasks (summarization)
    /// - Standard: General tasks (conversation)
    /// - Premium: Complex tasks (planning, code generation, code review)
    #[must_use]
    pub fn recommended_tier(&self) -> ModelTier {
        match self {
            // UltraBudget: Trivial tasks that don't need advanced reasoning
            Self::Classification => ModelTier::UltraBudget,
            Self::Extraction => ModelTier::UltraBudget,
            Self::Translation => ModelTier::UltraBudget,

            // Fast: Simple tasks requiring basic comprehension
            Self::Summarization => ModelTier::Fast,

            // Standard: General tasks requiring good language understanding
            Self::Conversation => ModelTier::Standard,

            // Premium: Complex tasks requiring advanced reasoning
            Self::Planning => ModelTier::Premium,
            Self::CodeGeneration => ModelTier::Premium,
            Self::CodeReview => ModelTier::Premium,
        }
    }

    /// Whether this task type requires tool support
    #[must_use]
    pub fn requires_tools(&self) -> bool {
        matches!(
            self,
            Self::Planning | Self::CodeGeneration | Self::CodeReview
        )
    }

    /// Get the default token budget for this task type
    ///
    /// Token budgets are optimized based on typical response lengths:
    /// - Classification: Short labels/categories (200 tokens)
    /// - Extraction: Structured data extraction (500 tokens)
    /// - Summarization: Condensed summaries (1000 tokens)
    /// - Translation: Similar length to input (800 tokens)
    /// - Conversation: General chat responses (2000 tokens)
    /// - Planning: Detailed step-by-step plans (3000 tokens)
    /// - CodeReview: Analysis with suggestions (3000 tokens)
    /// - CodeGeneration: Full code implementations (4096 tokens)
    #[must_use]
    pub fn default_token_budget(&self) -> TokenBudget {
        match self {
            Self::Classification => TokenBudget::new(200, 0.3),
            Self::Extraction => TokenBudget::new(500, 0.2),
            Self::Summarization => TokenBudget::new(1000, 0.5),
            Self::Translation => TokenBudget::new(800, 0.3),
            Self::Conversation => TokenBudget::new(2000, 0.7),
            Self::Planning => TokenBudget::new(3000, 0.7),
            Self::CodeReview => TokenBudget::new(3000, 0.5),
            Self::CodeGeneration => TokenBudget::new(4096, 0.7),
        }
    }
}

// ============================================================================
// Model Tier
// ============================================================================

/// Model tier for cost/performance optimization
///
/// Tiers are ordered by cost (ascending) and quality (ascending):
/// - UltraBudget: < $0.15/M tokens (DeepSeek R1 Distill, GPT-5 nano)
/// - Fast: $0.15 ~ $1.00/M tokens (GPT-5 nano, Gemini 2.5 Flash)
/// - Standard: $1.00 ~ $5.00/M tokens (Claude Sonnet 4.5, GPT-5)
/// - Premium: > $5.00/M tokens (Claude Opus 4.5)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    /// Ultra-budget models for trivial tasks (< $0.15/M tokens)
    /// Examples: DeepSeek R1 Distill ($0.03), Qwen 2.5-VL ($0.05), GLM-4-9B ($0.086)
    UltraBudget,
    /// Fast, cheap models for simple tasks ($0.15 ~ $1.00/M tokens)
    /// Examples: GPT-5 nano, Gemini 2.5 Flash
    Fast,
    /// Balanced models for general tasks ($1.00 ~ $5.00/M tokens)
    /// Examples: Claude Sonnet 4.5, GPT-5, Gemini 2.5 Pro
    Standard,
    /// Premium models for complex reasoning (> $5.00/M tokens)
    /// Examples: Claude Opus 4.5
    Premium,
}

impl ModelTier {
    /// Get default model for each provider at this tier
    ///
    /// Model selection follows 2026 pricing tiers:
    /// - UltraBudget: DeepSeek R1 Distill, Qwen 2.5, GLM-4-9B
    /// - Fast: GPT-5 nano, Gemini 2.5 Flash, Groq GPT-OSS
    /// - Standard: Claude Sonnet 4.5, GPT-5, Gemini 2.5 Pro
    /// - Premium: Claude Opus 4.5
    #[must_use]
    pub fn default_model(&self, provider: &str) -> &'static str {
        match (self, provider) {
            // ================================================================
            // DeepSeek - Ultra-low-cost leader ($0.03 ~ $0.55/M tokens)
            // ================================================================
            (ModelTier::UltraBudget, "deepseek") => "deepseek-r1-distill-llama-70b",
            (ModelTier::Fast, "deepseek") => "deepseek-chat",
            (ModelTier::Standard, "deepseek") => "deepseek-chat",
            (ModelTier::Premium, "deepseek") => "deepseek-reasoner",

            // ================================================================
            // Groq - FREE tier (rate limited)
            // ================================================================
            (ModelTier::UltraBudget, "groq") => "llama-3.1-8b-instant",
            (ModelTier::Fast, "groq") => "openai/gpt-oss-20b",
            (ModelTier::Standard, "groq") => "openai/gpt-oss-120b",
            (ModelTier::Premium, "groq") => "openai/gpt-oss-120b",

            // ================================================================
            // OpenAI - GPT-5 family (Aug 2025~)
            // gpt-5-nano: $0.05/$0.40, gpt-5: $1.25/$10.00
            // ================================================================
            (ModelTier::UltraBudget, "openai") => "gpt-5-nano",
            (ModelTier::Fast, "openai") => "gpt-5-nano",
            (ModelTier::Standard, "openai") => "gpt-5",
            (ModelTier::Premium, "openai") => "gpt-5",

            // ================================================================
            // Anthropic - Claude 4.5 family
            // haiku-4.5: $1/$5, sonnet-4.5: $3/$15, opus-4.5: $5/$25
            // ================================================================
            (ModelTier::UltraBudget, "anthropic") => "claude-haiku-4-5-20251001",
            (ModelTier::Fast, "anthropic") => "claude-haiku-4-5-20251001",
            (ModelTier::Standard, "anthropic") => "claude-sonnet-4-5-20250929",
            (ModelTier::Premium, "anthropic") => "claude-opus-4-5-20250514",

            // ================================================================
            // Google Gemini - Gemini 2.5 family
            // ================================================================
            (ModelTier::UltraBudget, "gemini" | "google_pro") => "gemini-2.5-flash-lite",
            (ModelTier::Fast, "gemini" | "google_pro") => "gemini-2.5-flash",
            (ModelTier::Standard, "gemini" | "google_pro") => "gemini-2.5-pro",
            (ModelTier::Premium, "gemini" | "google_pro") => "gemini-2.5-pro",

            // ================================================================
            // GLM (ZhipuAI) - Chinese models
            // ================================================================
            (ModelTier::UltraBudget, "glm") => "glm-4.7-flash",
            (ModelTier::Fast, "glm") => "glm-4.7-flash",
            (ModelTier::Standard, "glm") => "glm-4.7",
            (ModelTier::Premium, "glm") => "glm-4-plus",

            // ================================================================
            // Qwen (Alibaba) - Qwen 3 family (2026)
            // ================================================================
            (ModelTier::UltraBudget, "qwen") => "qwen3-8b",
            (ModelTier::Fast, "qwen") => "qwen-turbo",
            (ModelTier::Standard, "qwen") => "qwen3-32b",
            (ModelTier::Premium, "qwen") => "qwen-max",

            // ================================================================
            // OpenRouter - Multi-provider gateway (no :free models — they're queue-based and unreliable)
            // ================================================================
            (ModelTier::UltraBudget, "openrouter") => "google/gemini-flash-1.5",
            (ModelTier::Fast, "openrouter") => "google/gemini-flash-1.5",
            (ModelTier::Standard, "openrouter") => "google/gemini-pro-1.5",
            (ModelTier::Premium, "openrouter") => "anthropic/claude-sonnet-4",

            // ================================================================
            // Novita - Free tier provider
            // ================================================================
            (ModelTier::UltraBudget, "novita") => "qwen/qwen2.5-7b-instruct",
            (ModelTier::Fast, "novita") => "qwen/qwen2.5-7b-instruct",
            (ModelTier::Standard, "novita") => "thudm/glm-4-9b-chat",
            (ModelTier::Premium, "novita") => "qwen/qwen2.5-72b-instruct",

            // ================================================================
            // SiliconFlow - Cheapest provider ($0.03 ~ $0.09/M tokens)
            // ================================================================
            (ModelTier::UltraBudget, "siliconflow") => "deepseek-ai/DeepSeek-R1-Distill-Llama-70B",
            (ModelTier::Fast, "siliconflow") => "Qwen/Qwen2.5-VL-7B-Instruct",
            (ModelTier::Standard, "siliconflow") => "meta-llama/Llama-3.1-70B-Instruct",
            (ModelTier::Premium, "siliconflow") => "meta-llama/Llama-3.1-70B-Instruct",

            // ================================================================
            // Fireworks - Fast inference
            // ================================================================
            (ModelTier::UltraBudget, "fireworks") => {
                "accounts/fireworks/models/llama-v3p1-8b-instruct"
            }
            (ModelTier::Fast, "fireworks") => "accounts/fireworks/models/llama-v3p1-70b-instruct",
            (ModelTier::Standard, "fireworks") => {
                "accounts/fireworks/models/llama-v3p1-405b-instruct"
            }
            (ModelTier::Premium, "fireworks") => {
                "accounts/fireworks/models/llama-v3p1-405b-instruct"
            }

            // ================================================================
            // Ollama - Local models (all same model)
            // ================================================================
            (_, "ollama") => "qwen2.5:7b",

            // ================================================================
            // Default fallback
            // ================================================================
            _ => "gpt-5",
        }
    }

    /// Estimated cost multiplier relative to Fast tier
    ///
    /// Based on 2026 pricing:
    /// - UltraBudget: ~$0.05/M → 0.2x
    /// - Fast: ~$0.50/M → 1.0x (baseline)
    /// - Standard: ~$3.00/M → 6.0x
    /// - Premium: ~$15.00/M → 30.0x
    #[must_use]
    pub fn cost_multiplier(&self) -> f32 {
        match self {
            ModelTier::UltraBudget => 0.1,
            ModelTier::Fast => 1.0,
            ModelTier::Standard => 6.0,
            ModelTier::Premium => 30.0,
        }
    }

    /// Get the price range description for this tier
    #[must_use]
    pub fn price_range(&self) -> &'static str {
        match self {
            ModelTier::UltraBudget => "< $0.15/M tokens",
            ModelTier::Fast => "$0.15 ~ $1.00/M tokens",
            ModelTier::Standard => "$1.00 ~ $5.00/M tokens",
            ModelTier::Premium => "> $5.00/M tokens",
        }
    }

    /// Constrain this tier to not exceed the given maximum tier
    ///
    /// Tier ordering: UltraBudget < Fast < Standard < Premium
    #[must_use]
    pub fn constrain_to(&self, max_tier: &ModelTier) -> ModelTier {
        let self_level = self.level();
        let max_level = max_tier.level();

        if self_level <= max_level {
            *self
        } else {
            *max_tier
        }
    }

    /// Get numeric level for tier comparison (lower = cheaper)
    #[must_use]
    fn level(&self) -> u8 {
        match self {
            ModelTier::UltraBudget => 0,
            ModelTier::Fast => 1,
            ModelTier::Standard => 2,
            ModelTier::Premium => 3,
        }
    }
}
