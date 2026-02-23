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

/// All 14 providers (2026-02 pricing).
pub const PROVIDERS: &[Provider] = &[
    // ── FREE ──
    Provider {
        name: "zhipu",
        display_en: "ZhipuAI GLM (GLM-4.7-Flash) - Completely free",
        display_ko: "ZhipuAI GLM (GLM-4.7-Flash) - 완전 무료",
        env_var: "ZHIPU_API_KEY",
        signup_url: "https://z.ai/manage-apikey/apikey-list",
        category: ProviderCategory::Free,
        cost_en: "Free (Flash models, no daily limit)",
        cost_ko: "무료 (Flash 모델, 일일 제한 없음)",
    },
    Provider {
        name: "google",
        display_en: "Google AI (Gemini 2.0 Flash) - Free tier",
        display_ko: "Google AI (Gemini 2.0 Flash) - 무료",
        env_var: "GOOGLE_API_KEY",
        signup_url: "https://aistudio.google.com/apikey",
        category: ProviderCategory::Free,
        cost_en: "Free (1,500 RPD / 15 RPM)",
        cost_ko: "무료 (일 1,500회 / 분 15회)",
    },
    Provider {
        name: "groq",
        display_en: "Groq - Ultra fast inference (Free tier)",
        display_ko: "Groq - 초고속 추론 (무료 가능)",
        env_var: "GROQ_API_KEY",
        signup_url: "https://console.groq.com/keys",
        category: ProviderCategory::Free,
        cost_en: "Free tier available, paid from $0.06/M",
        cost_ko: "무료 가능, 유료 $0.06/M~",
    },
    Provider {
        name: "novita",
        display_en: "Novita AI - Free models available",
        display_ko: "Novita AI - 무료 모델 제공",
        env_var: "NOVITA_API_KEY",
        signup_url: "https://novita.ai",
        category: ProviderCategory::Free,
        cost_en: "Free (Qwen2.5-7B, GLM-4-9B)",
        cost_ko: "무료 (Qwen2.5-7B, GLM-4-9B)",
    },
    Provider {
        name: "siliconflow",
        display_en: "SiliconFlow - Free & ultra-cheap models",
        display_ko: "SiliconFlow - 무료 및 초저가",
        env_var: "SILICONFLOW_API_KEY",
        signup_url: "https://siliconflow.cn",
        category: ProviderCategory::Free,
        cost_en: "Free (Qwen2.5-7B), paid from $0.05/M",
        cost_ko: "무료 (Qwen2.5-7B), 유료 $0.05/M~",
    },
    // ── PAID (low-cost) ──
    Provider {
        name: "deepseek",
        display_en: "DeepSeek (V3) - Ultra cheap",
        display_ko: "DeepSeek (V3) - 초저가",
        env_var: "DEEPSEEK_API_KEY",
        signup_url: "https://platform.deepseek.com",
        category: ProviderCategory::Paid,
        cost_en: "$0.14/$0.28/M (input/output)",
        cost_ko: "$0.14/$0.28/M (입력/출력)",
    },
    Provider {
        name: "qwen",
        display_en: "Qwen (Alibaba, Qwen3.5)",
        display_ko: "Qwen (알리바바, Qwen3.5)",
        env_var: "DASHSCOPE_API_KEY",
        signup_url: "https://dashscope.console.aliyun.com",
        category: ProviderCategory::Paid,
        cost_en: "From $0.06/M (Turbo)",
        cost_ko: "$0.06/M~ (Turbo)",
    },
    Provider {
        name: "fireworks",
        display_en: "Fireworks (Fast inference)",
        display_ko: "Fireworks (빠른 추론)",
        env_var: "FIREWORKS_API_KEY",
        signup_url: "https://fireworks.ai",
        category: ProviderCategory::Paid,
        cost_en: "From $0.20/M (Qwen3 8B)",
        cost_ko: "$0.20/M~ (Qwen3 8B)",
    },
    Provider {
        name: "moonshot",
        display_en: "Moonshot AI (Kimi K2.5)",
        display_ko: "Moonshot AI (Kimi K2.5)",
        env_var: "MOONSHOT_API_KEY",
        signup_url: "https://platform.moonshot.ai",
        category: ProviderCategory::Paid,
        cost_en: "$0.60/$2.50/M (input/output)",
        cost_ko: "$0.60/$2.50/M (입력/출력)",
    },
    Provider {
        name: "openrouter",
        display_en: "OpenRouter - Multi-model gateway (100+ models)",
        display_ko: "OpenRouter - 멀티모델 게이트웨이 (100+ 모델)",
        env_var: "OPENROUTER_API_KEY",
        signup_url: "https://openrouter.ai/keys",
        category: ProviderCategory::Paid,
        cost_en: "Pay-per-use, varies by model",
        cost_ko: "모델별 종량제",
    },
    // ── PAID (premium) ──
    Provider {
        name: "google_pro",
        display_en: "Google AI Pro (Gemini 2.5 Pro) - High quotas",
        display_ko: "Google AI Pro (Gemini 2.5 Pro) - 높은 할당량",
        env_var: "GOOGLE_API_KEY",
        signup_url: "https://aistudio.google.com/apikey",
        category: ProviderCategory::Paid,
        cost_en: "$1.25/$15.00/M (input/output)",
        cost_ko: "$1.25/$15.00/M (입력/출력)",
    },
    Provider {
        name: "openai",
        display_en: "OpenAI (GPT-5)",
        display_ko: "OpenAI (GPT-5)",
        env_var: "OPENAI_API_KEY",
        signup_url: "https://platform.openai.com/api-keys",
        category: ProviderCategory::Paid,
        cost_en: "$1.25/$10.00/M (input/output)",
        cost_ko: "$1.25/$10.00/M (입력/출력)",
    },
    Provider {
        name: "anthropic",
        display_en: "Anthropic (Claude Sonnet 4.5)",
        display_ko: "Anthropic (Claude Sonnet 4.5)",
        env_var: "ANTHROPIC_API_KEY",
        signup_url: "https://console.anthropic.com",
        category: ProviderCategory::Paid,
        cost_en: "$3/$15/M (input/output)",
        cost_ko: "$3/$15/M (입력/출력)",
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
