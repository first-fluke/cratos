//! Cratos Wizard - User-friendly multilingual setup wizard
//!
//! A beginner-friendly setup wizard with:
//! - Automatic language detection (en/ko)
//! - Step-by-step instructions with clickable links
//! - Clear guidance for non-developers

use inquire::{Confirm, Password, Select};
use std::fs;
use std::path::Path;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Korean,
}

impl Language {
    #[allow(dead_code)]
    pub fn code(&self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Korean => "ko",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code.to_lowercase().as_str() {
            "ko" | "kr" | "korean" => Self::Korean,
            _ => Self::English,
        }
    }
}

/// Detect system language
pub fn detect_language() -> Language {
    // 1. Check environment variables: LANG, LC_ALL, LC_MESSAGES
    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var) {
            let lang = value.to_lowercase();
            if lang.starts_with("ko") {
                return Language::Korean;
            }
        }
    }

    // 2. macOS: defaults read -g AppleLocale
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLocale"])
            .output()
        {
            if let Ok(locale) = String::from_utf8(output.stdout) {
                if locale.to_lowercase().starts_with("ko") {
                    return Language::Korean;
                }
            }
        }
    }

    // 3. Windows: check via environment or registry (simplified)
    #[cfg(target_os = "windows")]
    {
        if let Ok(value) = std::env::var("LANG") {
            if value.to_lowercase().starts_with("ko") {
                return Language::Korean;
            }
        }
    }

    // 4. Default: English
    Language::English
}

/// Wizard text strings for internationalization
#[allow(dead_code)]
pub struct WizardTexts {
    // Welcome
    pub welcome_title: &'static str,
    pub welcome_subtitle: &'static str,
    pub welcome_steps: &'static str,
    pub welcome_time: &'static str,

    // Step 1: Telegram
    pub step1_title: &'static str,
    pub step1_desc: &'static str,
    pub step1_instructions: &'static str,
    pub step1_link: &'static str,
    pub step1_help_link: &'static str,
    pub step1_prompt: &'static str,
    pub step1_skip: &'static str,
    pub step1_skip_note: &'static str,

    // Step 2: LLM Provider
    pub step2_title: &'static str,
    pub step2_desc: &'static str,
    pub step2_free_header: &'static str,
    pub step2_paid_header: &'static str,
    pub step2_prompt: &'static str,

    // Step 3: API Key
    pub step3_title: &'static str,
    pub step3_instructions: &'static str,
    pub step3_prompt: &'static str,

    // Step 4: Test
    pub step4_title: &'static str,
    pub step4_testing_telegram: &'static str,
    pub step4_testing_llm: &'static str,
    pub step4_success: &'static str,
    pub step4_failed: &'static str,
    pub step4_continue_anyway: &'static str,

    // Complete
    pub complete_title: &'static str,
    pub complete_saved: &'static str,
    pub complete_summary: &'static str,
    pub complete_next_steps: &'static str,
    pub complete_tips: &'static str,
    pub complete_problems: &'static str,

    // Common
    pub yes: &'static str,
    pub no: &'static str,
    pub enabled: &'static str,
    pub disabled: &'static str,
    pub cancel_msg: &'static str,
}

/// English texts
pub const TEXTS_EN: WizardTexts = WizardTexts {
    // Welcome
    welcome_title: "Welcome to Cratos Setup Wizard!",
    welcome_subtitle: "Let's set up your AI assistant in just a few minutes.",
    welcome_steps: r#"
Setup consists of 3 steps:
  1. Connect Telegram Bot (5 min)
  2. Choose AI Model (2 min)
  3. Test Connection (1 min)
"#,
    welcome_time: "Total time: ~8 minutes",

    // Step 1: Telegram
    step1_title: "Step 1: Create Telegram Bot",
    step1_desc: "You need a Telegram bot to chat with Cratos.",
    step1_instructions: r#"
How to create:
  1. Click the link below (opens Telegram app):
     https://t.me/BotFather

  2. Send /newbot to BotFather

  3. Enter bot name (e.g., My AI Assistant)

  4. Enter bot username (e.g., my_ai_bot)
     Must end with _bot!

  5. Copy the token you receive (e.g., 123456789:ABCdefGHI...)
"#,
    step1_link: "https://t.me/BotFather",
    step1_help_link: "Help: https://core.telegram.org/bots#how-do-i-create-a-bot",
    step1_prompt: "Paste your Telegram bot token:",
    step1_skip: "Skip Telegram setup?",
    step1_skip_note: "(You can set this up later)",

    // Step 2: LLM Provider
    step2_title: "Step 2: Choose AI Model",
    step2_desc: "Choose which AI to use.\nIf you're new, we recommend a free option!",
    step2_free_header: "FREE OPTIONS (Recommended)",
    step2_paid_header: "PAID OPTIONS (Higher quality)",
    step2_prompt: "Select an AI provider (enter number):",

    // Step 3: API Key
    step3_title: "Get API Key",
    step3_instructions: r#"
How to get:
  1. Sign up/log in at the link below:
     {url}

  2. Click "Create Key" or "Generate API Key"

  3. Copy the generated key
"#,
    step3_prompt: "Paste your API key:",

    // Step 4: Test
    step4_title: "Step 3: Test Connection",
    step4_testing_telegram: "Testing Telegram connection...",
    step4_testing_llm: "Testing LLM connection...",
    step4_success: "Success!",
    step4_failed: "Failed. Please verify your credentials.",
    step4_continue_anyway: "Continue anyway?",

    // Complete
    complete_title: "Setup Complete!",
    complete_saved: "Configuration saved to .env",
    complete_summary: "Summary:",
    complete_next_steps: r#"
Next steps:
  1. Start Cratos:
     $ cratos serve

  2. Open Telegram and search for your bot

  3. Start chatting: /start

  4. Try: "What's the weather today?"
"#,
    complete_tips: r#"
Tips:
  - Run diagnostics: cratos doctor
  - Switch personas: @sindri, @athena, @heimdall
  - View help: cratos --help
"#,
    complete_problems: "Having problems? Run: cratos doctor",

    // Common
    yes: "Yes",
    no: "No",
    enabled: "enabled",
    disabled: "disabled",
    cancel_msg: "Setup cancelled.",
};

/// Korean texts
pub const TEXTS_KO: WizardTexts = WizardTexts {
    // Welcome
    welcome_title: "Cratos 설정 마법사에 오신 것을 환영합니다!",
    welcome_subtitle: "몇 분만에 AI 어시스턴트를 설정해 보세요.",
    welcome_steps: r#"
설정은 3단계로 진행됩니다:
  1. Telegram 봇 연결 (5분)
  2. AI 모델 선택 (2분)
  3. 연결 테스트 (1분)
"#,
    welcome_time: "총 소요 시간: 약 8분",

    // Step 1: Telegram
    step1_title: "1단계: Telegram 봇 만들기",
    step1_desc: "Telegram 봇을 만들어야 Cratos와 대화할 수 있어요.",
    step1_instructions: r#"
따라하기:
  1. 아래 링크를 클릭하세요 (Telegram 앱이 열려요):
     https://t.me/BotFather

  2. BotFather에게 /newbot 입력

  3. 봇 이름 입력 (예: 내 AI 비서)

  4. 봇 사용자명 입력 (예: my_ai_bot)
     반드시 _bot으로 끝나야 해요!

  5. 받은 토큰 복사 (예: 123456789:ABCdefGHI...)
"#,
    step1_link: "https://t.me/BotFather",
    step1_help_link: "도움말: https://core.telegram.org/bots#how-do-i-create-a-bot",
    step1_prompt: "Telegram 봇 토큰을 붙여넣기 하세요:",
    step1_skip: "Telegram 설정 건너뛰기?",
    step1_skip_note: "(나중에 설정할 수 있어요)",

    // Step 2: LLM Provider
    step2_title: "2단계: AI 모델 선택",
    step2_desc: "어떤 AI를 사용할지 선택하세요.\n처음이라면 무료 옵션을 추천해요!",
    step2_free_header: "무료 옵션 (추천)",
    step2_paid_header: "유료 옵션 (고품질)",
    step2_prompt: "사용할 AI를 선택하세요 (숫자 입력):",

    // Step 3: API Key
    step3_title: "API 키 발급",
    step3_instructions: r#"
따라하기:
  1. 아래 링크에서 가입/로그인:
     {url}

  2. "Create Key" 또는 "API 키 생성" 버튼 클릭

  3. 생성된 키 복사
"#,
    step3_prompt: "API 키를 붙여넣기 하세요:",

    // Step 4: Test
    step4_title: "3단계: 연결 테스트",
    step4_testing_telegram: "Telegram 연결 확인 중...",
    step4_testing_llm: "LLM 연결 확인 중...",
    step4_success: "성공!",
    step4_failed: "실패. 인증 정보를 확인해 주세요.",
    step4_continue_anyway: "그래도 계속할까요?",

    // Complete
    complete_title: "설정 완료!",
    complete_saved: "설정이 .env 파일에 저장되었습니다",
    complete_summary: "요약:",
    complete_next_steps: r#"
다음 단계:
  1. Cratos 실행:
     $ cratos serve

  2. Telegram에서 내 봇 검색

  3. 대화 시작: /start

  4. 시도해 보세요: "오늘 날씨 어때?"
"#,
    complete_tips: r#"
팁:
  - 진단 실행: cratos doctor
  - 페르소나 전환: @sindri, @athena, @heimdall
  - 도움말 보기: cratos --help
"#,
    complete_problems: "문제가 있으면 실행: cratos doctor",

    // Common
    yes: "예",
    no: "아니오",
    enabled: "활성화",
    disabled: "비활성화",
    cancel_msg: "설정이 취소되었습니다.",
};

/// Get texts for a language
pub fn get_texts(lang: Language) -> &'static WizardTexts {
    match lang {
        Language::English => &TEXTS_EN,
        Language::Korean => &TEXTS_KO,
    }
}

/// LLM Provider information for wizard
struct WizardProvider {
    name: &'static str,
    display_en: &'static str,
    display_ko: &'static str,
    env_var: &'static str,
    signup_url: &'static str,
    is_free: bool,
    cost_info_en: &'static str,
    cost_info_ko: &'static str,
}

const WIZARD_PROVIDERS: &[WizardProvider] = &[
    // Free options
    WizardProvider {
        name: "openrouter",
        display_en: "OpenRouter - Free models (Llama, Mistral)",
        display_ko: "OpenRouter - 무료 모델 (Llama, Mistral)",
        env_var: "OPENROUTER_API_KEY",
        signup_url: "https://openrouter.ai/keys",
        is_free: true,
        cost_info_en: "Free tier available",
        cost_info_ko: "무료 사용 가능",
    },
    WizardProvider {
        name: "groq",
        display_en: "Groq - Ultra fast, free (Llama)",
        display_ko: "Groq - 초고속, 무료 (Llama)",
        env_var: "GROQ_API_KEY",
        signup_url: "https://console.groq.com/keys",
        is_free: true,
        cost_info_en: "Free tier available",
        cost_info_ko: "무료 사용 가능",
    },
    WizardProvider {
        name: "google",
        display_en: "Google AI (Gemini) - Generous free tier",
        display_ko: "Google AI (Gemini) - 넉넉한 무료",
        env_var: "GOOGLE_API_KEY",
        signup_url: "https://aistudio.google.com/apikey",
        is_free: true,
        cost_info_en: "Free tier available",
        cost_info_ko: "무료 사용 가능",
    },
    // Paid options
    WizardProvider {
        name: "openai",
        display_en: "OpenAI (ChatGPT)",
        display_ko: "OpenAI (ChatGPT)",
        env_var: "OPENAI_API_KEY",
        signup_url: "https://platform.openai.com/api-keys",
        is_free: false,
        cost_info_en: "~$0.01/1000 tokens",
        cost_info_ko: "~$0.01/1000 토큰",
    },
    WizardProvider {
        name: "anthropic",
        display_en: "Anthropic (Claude)",
        display_ko: "Anthropic (Claude)",
        env_var: "ANTHROPIC_API_KEY",
        signup_url: "https://console.anthropic.com",
        is_free: false,
        cost_info_en: "~$0.015/1000 tokens",
        cost_info_ko: "~$0.015/1000 토큰",
    },
    WizardProvider {
        name: "deepseek",
        display_en: "DeepSeek - Very cheap",
        display_ko: "DeepSeek - 매우 저렴",
        env_var: "DEEPSEEK_API_KEY",
        signup_url: "https://platform.deepseek.com",
        is_free: false,
        cost_info_en: "~$0.001/1000 tokens",
        cost_info_ko: "~$0.001/1000 토큰",
    },
    WizardProvider {
        name: "ollama",
        display_en: "Ollama (Local, free, requires setup)",
        display_ko: "Ollama (로컬, 무료, 설치 필요)",
        env_var: "",
        signup_url: "https://ollama.ai",
        is_free: true,
        cost_info_en: "Free (runs locally)",
        cost_info_ko: "무료 (로컬 실행)",
    },
];

/// Print a styled header
fn print_header(text: &str) {
    let line = "━".repeat(60);
    println!();
    println!("  {}", line);
    println!("  {}", text);
    println!("  {}", line);
    println!();
}

/// Print a styled box
fn print_box(title: &str, content: &str) {
    println!();
    println!("  ╔═══════════════════════════════════════════════════════════════╗");
    println!("  ║  {:^61}║", title);
    println!("  ╚═══════════════════════════════════════════════════════════════╝");
    for line in content.lines() {
        if !line.is_empty() {
            println!("  {}", line);
        }
    }
    println!();
}

/// Run the wizard
pub async fn run(lang_override: Option<&str>) -> anyhow::Result<()> {
    // Detect or use specified language
    let lang = match lang_override {
        Some(code) => Language::from_code(code),
        None => detect_language(),
    };

    let texts = get_texts(lang);

    // Welcome
    print_box(texts.welcome_title, texts.welcome_subtitle);
    println!("{}", texts.welcome_steps);
    println!("  {}\n", texts.welcome_time);

    // Check for existing .env
    let env_path = Path::new(".env");
    if env_path.exists() {
        let overwrite = Confirm::new(if lang == Language::Korean {
            ".env 파일이 이미 존재합니다. 덮어쓸까요?"
        } else {
            ".env file already exists. Overwrite?"
        })
        .with_default(false)
        .prompt()?;

        if !overwrite {
            println!("\n  {}", texts.cancel_msg);
            return Ok(());
        }
    }

    // Step 1: Telegram
    print_header(texts.step1_title);
    println!("  {}", texts.step1_desc);
    println!("{}", texts.step1_instructions);
    println!("  {}", texts.step1_help_link);
    println!();

    let skip_telegram = Confirm::new(texts.step1_skip)
        .with_default(false)
        .with_help_message(texts.step1_skip_note)
        .prompt()?;

    let telegram_token = if skip_telegram {
        String::new()
    } else {
        Password::new(texts.step1_prompt)
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .prompt()?
    };

    // Step 2: LLM Provider Selection
    print_header(texts.step2_title);
    println!("  {}", texts.step2_desc);
    println!();

    // Build provider list with categories
    let mut options: Vec<String> = Vec::new();
    let mut provider_indices: Vec<usize> = Vec::new();

    // Free options header
    options.push(format!("── {} ──", texts.step2_free_header));
    provider_indices.push(usize::MAX); // Marker for header

    for (idx, provider) in WIZARD_PROVIDERS.iter().enumerate() {
        if provider.is_free && provider.name != "ollama" {
            let display = if lang == Language::Korean {
                provider.display_ko
            } else {
                provider.display_en
            };
            let cost = if lang == Language::Korean {
                provider.cost_info_ko
            } else {
                provider.cost_info_en
            };
            options.push(format!("  {} ({})", display, cost));
            provider_indices.push(idx);
        }
    }

    // Paid options header
    options.push(format!("── {} ──", texts.step2_paid_header));
    provider_indices.push(usize::MAX);

    for (idx, provider) in WIZARD_PROVIDERS.iter().enumerate() {
        if !provider.is_free {
            let display = if lang == Language::Korean {
                provider.display_ko
            } else {
                provider.display_en
            };
            let cost = if lang == Language::Korean {
                provider.cost_info_ko
            } else {
                provider.cost_info_en
            };
            options.push(format!("  {} ({})", display, cost));
            provider_indices.push(idx);
        }
    }

    // Local option
    options.push(format!("── {} ──", "LOCAL"));
    provider_indices.push(usize::MAX);

    let ollama_idx = WIZARD_PROVIDERS
        .iter()
        .position(|p| p.name == "ollama")
        .unwrap();
    let ollama = &WIZARD_PROVIDERS[ollama_idx];
    let ollama_display = if lang == Language::Korean {
        ollama.display_ko
    } else {
        ollama.display_en
    };
    options.push(format!("  {}", ollama_display));
    provider_indices.push(ollama_idx);

    let selected = Select::new(texts.step2_prompt, options)
        .with_help_message(if lang == Language::Korean {
            "화살표로 선택, Enter로 확인"
        } else {
            "Use arrows to select, Enter to confirm"
        })
        .prompt()?;

    // Re-parse the selection to find provider index
    let provider_idx = provider_indices
        .iter()
        .find_map(|&idx| {
            if idx != usize::MAX {
                let provider = &WIZARD_PROVIDERS[idx];
                let display = if lang == Language::Korean {
                    provider.display_ko
                } else {
                    provider.display_en
                };
                if selected.contains(display) || selected.contains(provider.name) {
                    return Some(idx);
                }
            }
            None
        })
        .unwrap_or(0);

    let provider = &WIZARD_PROVIDERS[provider_idx];

    // Step 3: API Key
    let api_key = if provider.name == "ollama" {
        println!();
        println!(
            "  {}",
            if lang == Language::Korean {
                "Ollama는 로컬에서 실행됩니다. API 키가 필요 없어요."
            } else {
                "Ollama runs locally. No API key needed."
            }
        );
        println!(
            "  {}",
            if lang == Language::Korean {
                "Ollama가 실행 중인지 확인하세요: ollama serve"
            } else {
                "Make sure Ollama is running: ollama serve"
            }
        );
        String::new()
    } else {
        print_header(&format!("{} - {}", texts.step3_title, provider.name));

        let instructions = texts.step3_instructions.replace("{url}", provider.signup_url);
        println!("{}", instructions);
        println!();

        Password::new(texts.step3_prompt)
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .with_validator(inquire::required!())
            .prompt()?
    };

    // Step 4: Test Connection
    print_header(texts.step4_title);

    let mut all_ok = true;

    // Test Telegram
    if !telegram_token.is_empty() {
        print!("  {} ", texts.step4_testing_telegram);
        let telegram_ok = test_telegram_token(&telegram_token).await;
        if telegram_ok {
            println!("{}", texts.step4_success);
        } else {
            println!("{}", texts.step4_failed);
            all_ok = false;
        }
    }

    // Test LLM
    if !api_key.is_empty() || provider.name == "ollama" {
        print!("  {} ", texts.step4_testing_llm);
        let llm_ok = test_llm_connection(provider, &api_key).await;
        if llm_ok {
            println!("{}", texts.step4_success);
        } else {
            println!("{}", texts.step4_failed);
            all_ok = false;
        }
    }

    if !all_ok {
        println!();
        let proceed = Confirm::new(texts.step4_continue_anyway)
            .with_default(false)
            .prompt()?;

        if !proceed {
            println!("\n  {}", texts.cancel_msg);
            return Ok(());
        }
    }

    // Save configuration
    let env_content = build_env_file(provider, &api_key, &telegram_token);
    fs::write(env_path, env_content)?;

    // Complete
    print_box(texts.complete_title, "");
    println!("  {}", texts.complete_saved);
    println!();
    println!("  {}", texts.complete_summary);
    println!(
        "    LLM: {} ({})",
        provider.name,
        if provider.is_free {
            texts.enabled
        } else {
            "paid"
        }
    );
    println!(
        "    Telegram: {}",
        if telegram_token.is_empty() {
            texts.disabled
        } else {
            texts.enabled
        }
    );
    println!("{}", texts.complete_next_steps);
    println!("{}", texts.complete_tips);
    println!("  {}", texts.complete_problems);
    println!();

    Ok(())
}

/// Test Telegram token by calling getMe API
async fn test_telegram_token(token: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let url = format!("https://api.telegram.org/bot{}/getMe", token);

    match client.get(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Test LLM provider connection
async fn test_llm_connection(provider: &WizardProvider, api_key: &str) -> bool {
    if provider.name == "ollama" {
        // Test Ollama by checking if it's running
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        match client.get("http://localhost:11434/api/tags").send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    } else {
        // For API providers, do a basic validation
        !api_key.is_empty() && api_key.len() > 10
    }
}

/// Build .env file content
fn build_env_file(provider: &WizardProvider, api_key: &str, telegram_token: &str) -> String {
    let mut content = String::from("# Cratos Environment Variables\n");
    content.push_str("# Generated by cratos wizard\n\n");

    // LLM Provider
    content.push_str("# ===================\n");
    content.push_str("# LLM Provider\n");
    content.push_str("# ===================\n");

    if provider.name == "ollama" {
        content.push_str("OLLAMA_BASE_URL=http://localhost:11434\n");
    } else {
        content.push_str(&format!("{}={}\n", provider.env_var, api_key));
    }

    content.push_str(&format!("CRATOS_DEFAULT_PROVIDER={}\n", provider.name));

    // Telegram
    content.push_str("\n# ===================\n");
    content.push_str("# Telegram\n");
    content.push_str("# ===================\n");
    if telegram_token.is_empty() {
        content.push_str("# TELEGRAM_BOT_TOKEN=your-telegram-bot-token\n");
    } else {
        content.push_str(&format!("TELEGRAM_BOT_TOKEN={}\n", telegram_token));
    }

    // Server
    content.push_str("\n# ===================\n");
    content.push_str("# Server\n");
    content.push_str("# ===================\n");
    content.push_str("HOST=127.0.0.1\n");
    content.push_str("PORT=9742\n");

    // Logging
    content.push_str("\n# ===================\n");
    content.push_str("# Logging\n");
    content.push_str("# ===================\n");
    content.push_str("RUST_LOG=cratos=info,tower_http=info\n");

    // Default persona
    content.push_str("\n# ===================\n");
    content.push_str("# Default Persona\n");
    content.push_str("# ===================\n");
    content.push_str("CRATOS_DEFAULT_PERSONA=cratos\n");

    content
}
