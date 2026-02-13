//! Tests for router module

use super::*;

#[test]
fn test_router() {
    let router = LlmRouter::new("openai");
    assert_eq!(router.default_provider_name(), "openai");
    assert!(!router.has_provider("openai"));
    assert!(router.list_providers().is_empty());
}

#[test]
fn test_task_type_recommended_tier() {
    // UltraBudget tier tasks (trivial)
    assert_eq!(
        TaskType::Classification.recommended_tier(),
        ModelTier::UltraBudget
    );
    assert_eq!(
        TaskType::Extraction.recommended_tier(),
        ModelTier::UltraBudget
    );
    assert_eq!(
        TaskType::Translation.recommended_tier(),
        ModelTier::UltraBudget
    );

    // Fast tier tasks (simple)
    assert_eq!(TaskType::Summarization.recommended_tier(), ModelTier::Fast);

    // Standard tier tasks (general)
    assert_eq!(
        TaskType::Conversation.recommended_tier(),
        ModelTier::Standard
    );

    // Premium tier tasks (complex)
    assert_eq!(TaskType::Planning.recommended_tier(), ModelTier::Premium);
    assert_eq!(
        TaskType::CodeGeneration.recommended_tier(),
        ModelTier::Premium
    );
    assert_eq!(TaskType::CodeReview.recommended_tier(), ModelTier::Premium);
}

#[test]
fn test_task_type_requires_tools() {
    assert!(TaskType::Planning.requires_tools());
    assert!(TaskType::CodeGeneration.requires_tools());
    assert!(TaskType::CodeReview.requires_tools());

    assert!(!TaskType::Classification.requires_tools());
    assert!(!TaskType::Summarization.requires_tools());
    assert!(!TaskType::Conversation.requires_tools());
}

#[test]
fn test_model_tier_default_model() {
    // OpenAI (GPT-5 family)
    assert_eq!(ModelTier::UltraBudget.default_model("openai"), "gpt-5-nano");
    assert_eq!(ModelTier::Fast.default_model("openai"), "gpt-5-nano");
    assert_eq!(ModelTier::Standard.default_model("openai"), "gpt-5");
    assert_eq!(ModelTier::Premium.default_model("openai"), "gpt-5");

    // Anthropic (Claude 4.5 family)
    assert_eq!(
        ModelTier::UltraBudget.default_model("anthropic"),
        "claude-haiku-4-5-20251001"
    );
    assert_eq!(
        ModelTier::Fast.default_model("anthropic"),
        "claude-haiku-4-5-20251001"
    );
    assert_eq!(
        ModelTier::Standard.default_model("anthropic"),
        "claude-sonnet-4-5-20250929"
    );
    assert_eq!(
        ModelTier::Premium.default_model("anthropic"),
        "claude-opus-4-5-20250514"
    );

    // Gemini (Gemini 2.5 family)
    assert_eq!(
        ModelTier::UltraBudget.default_model("gemini"),
        "gemini-2.5-flash-lite"
    );
    assert_eq!(ModelTier::Fast.default_model("gemini"), "gemini-2.5-flash");
    assert_eq!(
        ModelTier::Standard.default_model("gemini"),
        "gemini-2.5-pro"
    );

    // DeepSeek (ultra-low-cost)
    assert_eq!(
        ModelTier::UltraBudget.default_model("deepseek"),
        "deepseek-r1-distill-llama-70b"
    );
    assert_eq!(ModelTier::Fast.default_model("deepseek"), "deepseek-chat");
    assert_eq!(
        ModelTier::Premium.default_model("deepseek"),
        "deepseek-reasoner"
    );

    // Groq (FREE)
    assert_eq!(
        ModelTier::UltraBudget.default_model("groq"),
        "llama-3.1-8b-instant"
    );
}

#[test]
fn test_model_tier_cost_multiplier() {
    assert_eq!(ModelTier::UltraBudget.cost_multiplier(), 0.1);
    assert_eq!(ModelTier::Fast.cost_multiplier(), 1.0);
    assert_eq!(ModelTier::Standard.cost_multiplier(), 6.0);
    assert_eq!(ModelTier::Premium.cost_multiplier(), 30.0);
}

#[test]
fn test_model_tier_constrain_to() {
    // Premium constrained to Standard should return Standard
    assert_eq!(
        ModelTier::Premium.constrain_to(&ModelTier::Standard),
        ModelTier::Standard
    );
    // Premium constrained to Fast should return Fast
    assert_eq!(
        ModelTier::Premium.constrain_to(&ModelTier::Fast),
        ModelTier::Fast
    );
    // Fast constrained to Premium should stay Fast
    assert_eq!(
        ModelTier::Fast.constrain_to(&ModelTier::Premium),
        ModelTier::Fast
    );
    // UltraBudget constrained to anything should stay UltraBudget
    assert_eq!(
        ModelTier::UltraBudget.constrain_to(&ModelTier::Fast),
        ModelTier::UltraBudget
    );
}

#[test]
fn test_model_tier_price_range() {
    assert_eq!(ModelTier::UltraBudget.price_range(), "< $0.15/M tokens");
    assert_eq!(ModelTier::Fast.price_range(), "$0.15 ~ $1.00/M tokens");
    assert_eq!(ModelTier::Standard.price_range(), "$1.00 ~ $5.00/M tokens");
    assert_eq!(ModelTier::Premium.price_range(), "> $5.00/M tokens");
}

#[test]
fn test_routing_rules_default() {
    let rules = RoutingRules::default();
    assert!(rules.task_providers.is_empty());
    assert!(rules.task_models.is_empty());
    assert!(rules.task_token_budgets.is_empty());
    assert!(!rules.prefer_local);
    assert!(rules.max_tier.is_none());
}

#[test]
fn test_task_type_default_token_budget() {
    // Fast tier tasks should have small budgets
    assert_eq!(
        TaskType::Classification.default_token_budget().max_tokens,
        200
    );
    assert_eq!(TaskType::Extraction.default_token_budget().max_tokens, 500);
    assert_eq!(
        TaskType::Summarization.default_token_budget().max_tokens,
        1000
    );
    assert_eq!(TaskType::Translation.default_token_budget().max_tokens, 800);

    // Standard tier
    assert_eq!(
        TaskType::Conversation.default_token_budget().max_tokens,
        2000
    );

    // Premium tier - larger budgets
    assert_eq!(TaskType::Planning.default_token_budget().max_tokens, 3000);
    assert_eq!(TaskType::CodeReview.default_token_budget().max_tokens, 3000);
    assert_eq!(
        TaskType::CodeGeneration.default_token_budget().max_tokens,
        4096
    );
}

#[test]
fn test_routing_rules_get_token_budget() {
    use crate::token::TokenBudget;
    let mut rules = RoutingRules::default();

    // Should return default budget
    let budget = rules.get_token_budget(TaskType::Classification);
    assert_eq!(budget.max_tokens, 200);

    // With custom override
    rules
        .task_token_budgets
        .insert(TaskType::Classification, TokenBudget::new(500, 0.5));
    let budget = rules.get_token_budget(TaskType::Classification);
    assert_eq!(budget.max_tokens, 500);
    assert_eq!(budget.temperature, 0.5);
}

#[test]
fn test_token_budget_temperatures() {
    // Low temperature for deterministic tasks
    assert!(TaskType::Classification.default_token_budget().temperature < 0.5);
    assert!(TaskType::Extraction.default_token_budget().temperature < 0.5);
    assert!(TaskType::Translation.default_token_budget().temperature < 0.5);

    // Higher temperature for creative tasks
    assert!(TaskType::Conversation.default_token_budget().temperature >= 0.7);
    assert!(TaskType::CodeGeneration.default_token_budget().temperature >= 0.7);
}

#[test]
fn test_router_with_routing_rules() {
    let rules = RoutingRules {
        prefer_local: true,
        max_tier: Some(ModelTier::Standard),
        ..Default::default()
    };

    let router = LlmRouter::new("openai").with_routing_rules(rules);
    assert!(router.routing_rules().prefer_local);
    assert_eq!(router.routing_rules().max_tier, Some(ModelTier::Standard));
}

#[test]
fn test_estimate_cost() {
    let router = LlmRouter::new("openai");

    // UltraBudget tier: 0.1 multiplier
    let ultra_budget_cost = router.estimate_cost(TaskType::Classification, 1000);
    assert_eq!(ultra_budget_cost, 0.1);

    // Fast tier: 1.0 multiplier
    let fast_cost = router.estimate_cost(TaskType::Summarization, 1000);
    assert_eq!(fast_cost, 1.0);

    // Standard tier: 6.0 multiplier
    let standard_cost = router.estimate_cost(TaskType::Conversation, 1000);
    assert_eq!(standard_cost, 6.0);

    // Premium tier: 30.0 multiplier
    let premium_cost = router.estimate_cost(TaskType::CodeGeneration, 1000);
    assert_eq!(premium_cost, 30.0);
}

#[test]
fn test_model_routing_config_get_for_task() {
    let config = ModelRoutingConfig::default();

    // Classification uses UltraBudget -> trivial config
    let trivial = config.get_for_task(TaskType::Classification);
    assert_eq!(trivial.provider, "deepseek");

    // Summarization uses Fast -> simple config
    let simple = config.get_for_task(TaskType::Summarization);
    assert_eq!(simple.provider, "gemini");

    // Conversation uses Standard -> general config
    let general = config.get_for_task(TaskType::Conversation);
    assert_eq!(general.provider, "anthropic");

    // CodeGeneration uses Premium -> complex config
    let complex = config.get_for_task(TaskType::CodeGeneration);
    assert_eq!(complex.provider, "anthropic");
}

#[test]
fn test_model_routing_config_free_tier() {
    let config = ModelRoutingConfig::free_tier();

    // Free tier should use Groq for trivial/simple tasks
    assert_eq!(config.trivial.provider, "groq");
    assert_eq!(config.simple.provider, "groq");
    // And DeepSeek for general/complex tasks
    assert_eq!(config.general.provider, "deepseek");
    assert_eq!(config.complex.provider, "deepseek");
}

#[test]
fn test_model_routing_config_estimate_monthly_cost() {
    let config = ModelRoutingConfig::free_tier();

    // With free tier config (Groq + DeepSeek), cost should be very low
    let cost = config.estimate_monthly_cost(
        1_000_000, // 1M trivial tokens (Groq FREE)
        1_000_000, // 1M simple tokens (Groq FREE)
        1_000_000, // 1M general tokens (DeepSeek $0.21/M)
        100_000,   // 100K complex tokens (DeepSeek $1.37/M)
    );

    // Expected: 0 + 0 + 0.21 + 0.137 = ~$0.35
    assert!(cost < 1.0, "Free tier should be under $1 for 3.1M tokens");
}
