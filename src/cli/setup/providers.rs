//! LLM provider definitions for setup wizard.
//!
//! Union of providers from both old init.rs and wizard.rs,
//! categorized as FREE / PAID / LOCAL.

use super::i18n::Language;

/// Category for display grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCategory {
    Free,
    Paid,
    Local,
}

/// A provider entry shown during setup.
pub struct Provider {
    pub name: &'static str,
    pub display_en: &'static str,
    pub display_ko: &'static str,
    pub env_var: &'static str,
    pub signup_url: &'static str,
    pub category: ProviderCategory,
    pub cost_en: &'static str,
    pub cost_ko: &'static str,
}

impl Provider {
    pub fn display(&self, lang: Language) -> &'static str {
        match lang {
            Language::English => self.display_en,
            Language::Korean => self.display_ko,
        }
    }

    pub fn cost(&self, lang: Language) -> &'static str {
        match lang {
            Language::English => self.cost_en,
            Language::Korean => self.cost_ko,
        }
    }
}

/// All 12 providers: union of init.rs (11) + wizard.rs (7), deduplicated.
pub const PROVIDERS: &[Provider] = &[
    // ── FREE ──
    Provider {
        name: "groq",
        display_en: "Groq - Ultra fast, free (Llama)",
        display_ko: "Groq - 초고속, 무료 (Llama)",
        env_var: "GROQ_API_KEY",
        signup_url: "https://console.groq.com/keys",
        category: ProviderCategory::Free,
        cost_en: "Free tier available",
        cost_ko: "무료 사용 가능",
    },
    Provider {
        name: "openrouter",
        display_en: "OpenRouter - Free models (Llama, Mistral)",
        display_ko: "OpenRouter - 무료 모델 (Llama, Mistral)",
        env_var: "OPENROUTER_API_KEY",
        signup_url: "https://openrouter.ai/keys",
        category: ProviderCategory::Free,
        cost_en: "Free tier available",
        cost_ko: "무료 사용 가능",
    },
    Provider {
        name: "google",
        display_en: "Google AI (Gemini) - Generous free tier",
        display_ko: "Google AI (Gemini) - 넉넉한 무료",
        env_var: "GOOGLE_API_KEY",
        signup_url: "https://aistudio.google.com/apikey",
        category: ProviderCategory::Free,
        cost_en: "Free tier available",
        cost_ko: "무료 사용 가능",
    },
    Provider {
        name: "novita",
        display_en: "Novita AI - Free tier available",
        display_ko: "Novita AI - 무료 사용 가능",
        env_var: "NOVITA_API_KEY",
        signup_url: "https://novita.ai",
        category: ProviderCategory::Free,
        cost_en: "Free tier available",
        cost_ko: "무료 사용 가능",
    },
    // ── PAID ──
    Provider {
        name: "siliconflow",
        display_en: "SiliconFlow (Cheapest: $0.03/M tokens)",
        display_ko: "SiliconFlow (최저가: $0.03/M 토큰)",
        env_var: "SILICONFLOW_API_KEY",
        signup_url: "https://siliconflow.cn",
        category: ProviderCategory::Paid,
        cost_en: "$0.03/M tokens",
        cost_ko: "$0.03/M 토큰",
    },
    Provider {
        name: "deepseek",
        display_en: "DeepSeek - Very cheap ($0.14/M)",
        display_ko: "DeepSeek - 매우 저렴 ($0.14/M)",
        env_var: "DEEPSEEK_API_KEY",
        signup_url: "https://platform.deepseek.com",
        category: ProviderCategory::Paid,
        cost_en: "$0.14/M tokens",
        cost_ko: "$0.14/M 토큰",
    },
    Provider {
        name: "fireworks",
        display_en: "Fireworks (Fast inference, Llama 4)",
        display_ko: "Fireworks (빠른 추론, Llama 4)",
        env_var: "FIREWORKS_API_KEY",
        signup_url: "https://fireworks.ai",
        category: ProviderCategory::Paid,
        cost_en: "$0.20/M tokens",
        cost_ko: "$0.20/M 토큰",
    },
    Provider {
        name: "moonshot",
        display_en: "Moonshot AI (Kimi 2.5)",
        display_ko: "Moonshot AI (Kimi 2.5)",
        env_var: "MOONSHOT_API_KEY",
        signup_url: "https://platform.moonshot.cn",
        category: ProviderCategory::Paid,
        cost_en: "$0.60/M tokens",
        cost_ko: "$0.60/M 토큰",
    },
    Provider {
        name: "openai",
        display_en: "OpenAI (GPT-4o)",
        display_ko: "OpenAI (GPT-4o)",
        env_var: "OPENAI_API_KEY",
        signup_url: "https://platform.openai.com/api-keys",
        category: ProviderCategory::Paid,
        cost_en: "~$5.00/M tokens",
        cost_ko: "~$5.00/M 토큰",
    },
    Provider {
        name: "anthropic",
        display_en: "Anthropic (Claude Sonnet 4)",
        display_ko: "Anthropic (Claude Sonnet 4)",
        env_var: "ANTHROPIC_API_KEY",
        signup_url: "https://console.anthropic.com",
        category: ProviderCategory::Paid,
        cost_en: "~$3.00/M tokens",
        cost_ko: "~$3.00/M 토큰",
    },
    Provider {
        name: "zhipu",
        display_en: "ZhipuAI GLM (GLM-4)",
        display_ko: "ZhipuAI GLM (GLM-4)",
        env_var: "ZHIPU_API_KEY",
        signup_url: "https://open.bigmodel.cn",
        category: ProviderCategory::Paid,
        cost_en: "$0.50/M tokens",
        cost_ko: "$0.50/M 토큰",
    },
    // ── LOCAL ──
    Provider {
        name: "ollama",
        display_en: "Ollama (Local, free, requires setup)",
        display_ko: "Ollama (로컬, 무료, 설치 필요)",
        env_var: "",
        signup_url: "https://ollama.ai",
        category: ProviderCategory::Local,
        cost_en: "Free (runs locally)",
        cost_ko: "무료 (로컬 실행)",
    },
];
