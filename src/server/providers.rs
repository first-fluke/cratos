//! LLM provider resolution
//!
//! Initializes and registers all available LLM providers.

use super::config::LlmConfig;
use anyhow::Result;
use cratos_llm::{
    AnthropicConfig, AnthropicProvider, DeepSeekConfig, DeepSeekProvider, GeminiConfig,
    GeminiProvider, GlmConfig, GlmProvider, GroqConfig, GroqProvider, LlmRouter, MoonshotConfig,
    MoonshotProvider, NovitaConfig, NovitaProvider, OllamaConfig, OllamaProvider, OpenAiConfig,
    OpenAiProvider, OpenRouterConfig, OpenRouterProvider, QwenConfig, QwenProvider,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Resolve and configure LLM providers based on available API keys
pub fn resolve_llm_provider(llm_config: &LlmConfig) -> Result<Arc<LlmRouter>> {
    let mut router = LlmRouter::new(&llm_config.default_provider);
    let mut registered_count = 0;
    let mut default_provider: Option<String> = None;

    if let Ok(config) = GroqConfig::from_env() {
        if let Ok(provider) = GroqProvider::new(config) {
            router.register("groq", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("groq".to_string());
            }
            info!("Registered Groq provider");
        }
    }
    if let Ok(config) = OpenRouterConfig::from_env() {
        if let Ok(provider) = OpenRouterProvider::new(config) {
            router.register("openrouter", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("openrouter".to_string());
            }
            info!("Registered OpenRouter provider");
        }
    }
    if let Ok(config) = NovitaConfig::from_env() {
        if let Ok(provider) = NovitaProvider::new(config) {
            router.register("novita", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("novita".to_string());
            }
            info!("Registered Novita provider (free tier)");
        }
    }
    if let Ok(config) = DeepSeekConfig::from_env() {
        if let Ok(provider) = DeepSeekProvider::new(config) {
            router.register("deepseek", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("deepseek".to_string());
            }
            info!("Registered DeepSeek provider (low cost)");
        }
    }
    match OpenAiConfig::from_env() {
        Ok(config) => {
            let auth_source = config.auth_source;
            cratos_llm::cli_auth::register_auth_source("openai", auth_source);
            let provider = OpenAiProvider::new(config);
            router.register("openai", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("openai".to_string());
            }
            info!("Registered OpenAI provider ({})", auth_source);
        }
        Err(e) => {
            debug!("OpenAI provider not available: {}", e);
        }
    }
    if let Ok(config) = AnthropicConfig::from_env() {
        if let Ok(provider) = AnthropicProvider::new(config) {
            router.register("anthropic", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("anthropic".to_string());
            }
            info!("Registered Anthropic provider");
        }
    }
    match GeminiConfig::from_env() {
        Ok(config) => {
            let auth_source = config.auth_source;
            cratos_llm::cli_auth::register_auth_source("gemini", auth_source);
            match GeminiProvider::new(config) {
                Ok(provider) => {
                    router.register("gemini", Arc::new(provider));
                    registered_count += 1;
                    if default_provider.is_none() {
                        default_provider = Some("gemini".to_string());
                    }
                    if auth_source == cratos_llm::cli_auth::AuthSource::GeminiCli {
                        warn!(
                            "Gemini CLI OAuth detected — routed to Standard API (safe). \
                             For higher quotas (2,000 RPD Flash): set GEMINI_API_KEY \
                             (https://aistudio.google.com/apikey)"
                        );
                    }
                    info!("Registered Gemini provider ({})", auth_source);
                }
                Err(e) => debug!("Gemini provider init failed: {}", e),
            }
        }
        Err(e) => debug!("Gemini config not available: {}", e),
    }
    if let Ok(config) = GlmConfig::from_env() {
        if let Ok(provider) = GlmProvider::new(config) {
            router.register("glm", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("glm".to_string());
            }
            info!("Registered GLM provider");
        }
    }
    if let Ok(config) = MoonshotConfig::from_env() {
        if let Ok(provider) = MoonshotProvider::new(config) {
            router.register("moonshot", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("moonshot".to_string());
            }
            info!("Registered Moonshot provider");
        }
    }
    if let Ok(config) = QwenConfig::from_env() {
        if let Ok(provider) = QwenProvider::new(config) {
            router.register("qwen", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("qwen".to_string());
            }
            info!("Registered Qwen provider");
        }
    }
    let ollama_config = OllamaConfig::from_env();
    if let Ok(provider) = OllamaProvider::new(ollama_config) {
        router.register("ollama", Arc::new(provider));
        registered_count += 1;
        if default_provider.is_none() {
            default_provider = Some("ollama".to_string());
        }
        info!("Registered Ollama provider (local)");
    }

    if registered_count == 0 {
        return Err(anyhow::anyhow!(
            "No LLM provider configured.\n\n\
             To fix this, run one of:\n\
               cratos init     # Interactive setup wizard (recommended)\n\
               cratos doctor   # Check your configuration\n\n\
             Or manually set one of these environment variables:\n\
               GROQ_API_KEY        # Free tier, recommended\n\
               OPENROUTER_API_KEY  # Free tier available\n\
               NOVITA_API_KEY      # Free tier\n\
               DEEPSEEK_API_KEY    # Ultra low cost\n\
               MOONSHOT_API_KEY    # Kimi K2\n\
               ZHIPU_API_KEY       # GLM-4.7\n\
               OPENAI_API_KEY\n\
               ANTHROPIC_API_KEY\n\n\
             Or use CLI subscription tokens:\n\
               gemini auth login   # Gemini CLI (Antigravity Pro)\n\
               codex auth login    # Codex CLI (ChatGPT Pro/Plus)"
        ));
    }

    // Normalize provider aliases (e.g., "google" -> "gemini")
    let normalized_provider = match llm_config.default_provider.as_str() {
        "google" => "gemini".to_string(),
        "zhipu" | "zhipuai" => "glm".to_string(),
        other => other.to_string(),
    };

    if normalized_provider == "auto" || normalized_provider.is_empty() {
        if let Some(dp) = default_provider {
            router.set_default(&dp);
            info!("Auto-selected default provider: {}", dp);
        } else if let Some(first) = router.list_providers().first() {
            let first = first.to_string();
            router.set_default(&first);
            info!("Auto-selected fallback provider: {}", first);
        }
    } else if router.has_provider(&normalized_provider) {
        router.set_default(&normalized_provider);
        info!("Default provider set to: {}", normalized_provider);
    } else {
        warn!(
            "Configured default provider '{}' not available, using auto-detected",
            normalized_provider
        );
        if let Some(dp) = default_provider {
            router.set_default(&dp);
        }
    }

    info!(
        "LLM Router initialized with {} providers: {:?}",
        registered_count,
        router.list_providers()
    );

    Ok(Arc::new(router))
}
