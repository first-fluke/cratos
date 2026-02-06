//! Language detection and text constants for setup wizard.

/// Supported languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Korean,
}

impl Language {
    pub fn from_code(code: &str) -> Self {
        match code.to_lowercase().as_str() {
            "ko" | "kr" | "korean" => Self::Korean,
            _ => Self::English,
        }
    }
}

/// Detect system language from environment / OS settings.
pub fn detect_language() -> Language {
    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var) {
            if value.to_lowercase().starts_with("ko") {
                return Language::Korean;
            }
        }
    }

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

    Language::English
}

// ---------------------------------------------------------------------------
// Text constants
// ---------------------------------------------------------------------------

pub struct Texts {
    // Welcome
    pub welcome_title: &'static str,
    pub welcome_subtitle: &'static str,
    pub welcome_steps: &'static str,
    pub welcome_time: &'static str,

    // Non-interactive
    pub non_interactive_title: &'static str,
    pub non_interactive_body: &'static str,

    // .env overwrite
    pub env_overwrite: &'static str,

    // Telegram
    pub telegram_title: &'static str,
    pub telegram_desc: &'static str,
    pub telegram_instructions: &'static str,
    pub telegram_help_link: &'static str,
    pub telegram_prompt: &'static str,
    pub telegram_skip: &'static str,
    pub telegram_skip_note: &'static str,

    // Slack
    pub slack_title: &'static str,
    pub slack_skip: &'static str,
    pub slack_skip_note: &'static str,
    pub slack_token_prompt: &'static str,
    pub slack_secret_prompt: &'static str,

    // LLM Provider
    pub provider_title: &'static str,
    pub provider_desc: &'static str,
    pub provider_free_header: &'static str,
    pub provider_paid_header: &'static str,
    pub provider_local_header: &'static str,
    pub provider_prompt: &'static str,

    // API Key
    pub apikey_title: &'static str,
    pub apikey_instructions: &'static str,
    pub apikey_prompt: &'static str,
    pub apikey_ollama_no_key_en: &'static str,
    pub apikey_ollama_run_en: &'static str,

    // Persona
    pub persona_title: &'static str,
    pub persona_prompt: &'static str,
    pub persona_help: &'static str,

    // Test
    pub test_title: &'static str,
    pub test_telegram: &'static str,
    pub test_llm: &'static str,
    pub test_success: &'static str,
    pub test_failed: &'static str,
    pub test_continue: &'static str,

    // Complete
    pub complete_title: &'static str,
    pub complete_saved: &'static str,
    pub complete_summary: &'static str,
    pub complete_next_steps: &'static str,
    pub complete_tips: &'static str,
    pub complete_problems: &'static str,

    // Common
    pub enabled: &'static str,
    pub disabled: &'static str,
    pub cancel_msg: &'static str,
}

pub const TEXTS_EN: Texts = Texts {
    welcome_title: "Welcome to Cratos Setup!",
    welcome_subtitle: "Let's set up your AI assistant in just a few minutes.",
    welcome_steps: r#"
Setup steps:
  1. Connect Telegram Bot (optional)
  2. Connect Slack Bot (optional)
  3. Choose AI Model
  4. Select Persona
  5. Test Connection
"#,
    welcome_time: "Total time: ~5 minutes",

    non_interactive_title: "Cratos installed successfully!",
    non_interactive_body: r#"  Interactive setup is required. Run in your terminal:

    cratos init          Interactive setup (recommended)
    cratos init --lang ko   한국어 설정

  Or create a .env file manually with:
    TELEGRAM_BOT_TOKEN, OPENAI_API_KEY, etc."#,

    env_overwrite: ".env file already exists. Overwrite?",

    telegram_title: "Telegram Bot Setup",
    telegram_desc: "You need a Telegram bot to chat with Cratos.",
    telegram_instructions: r#"
How to create:
  1. Open Telegram and search for @BotFather
     https://t.me/BotFather

  2. Send /newbot to BotFather

  3. Enter bot name (e.g., My AI Assistant)

  4. Enter bot username (must end with _bot)

  5. Copy the token (e.g., 123456789:ABCdefGHI...)
"#,
    telegram_help_link: "Help: https://core.telegram.org/bots#how-do-i-create-a-bot",
    telegram_prompt: "Paste your Telegram bot token:",
    telegram_skip: "Skip Telegram setup?",
    telegram_skip_note: "(You can set this up later)",

    slack_title: "Slack Bot Setup",
    slack_skip: "Skip Slack setup?",
    slack_skip_note: "(You can set this up later)",
    slack_token_prompt: "Enter Slack bot token (xoxb-...):",
    slack_secret_prompt: "Enter Slack signing secret:",

    provider_title: "Choose AI Model",
    provider_desc: "Choose which AI to use.\nIf you're new, we recommend a free option!",
    provider_free_header: "FREE (Recommended)",
    provider_paid_header: "PAID (Higher quality)",
    provider_local_header: "LOCAL",
    provider_prompt: "Select an AI provider:",

    apikey_title: "Get API Key",
    apikey_instructions: r#"
How to get:
  1. Sign up / log in at:
     {url}

  2. Click "Create Key" or "Generate API Key"

  3. Copy the generated key
"#,
    apikey_prompt: "Paste your API key:",
    apikey_ollama_no_key_en: "Ollama runs locally. No API key needed.",
    apikey_ollama_run_en: "Make sure Ollama is running: ollama serve",

    persona_title: "Select Default Persona",
    persona_prompt: "Select default persona:",
    persona_help: "You can switch personas with @mention (e.g., @sindri)",

    test_title: "Test Connection",
    test_telegram: "Testing Telegram connection...",
    test_llm: "Testing LLM connection...",
    test_success: "Success!",
    test_failed: "Failed. Please verify your credentials.",
    test_continue: "Continue anyway?",

    complete_title: "Setup Complete!",
    complete_saved: "Configuration saved to .env",
    complete_summary: "Summary:",
    complete_next_steps: r#"
Next steps:
  1. Start Cratos:       cratos serve
  2. Open Telegram and search for your bot
  3. Start chatting:     /start
"#,
    complete_tips: r#"
Tips:
  - Run diagnostics:     cratos doctor
  - Switch personas:     @sindri, @athena, @heimdall
  - View help:           cratos --help
"#,
    complete_problems: "Having problems? Run: cratos doctor",

    enabled: "enabled",
    disabled: "disabled",
    cancel_msg: "Setup cancelled.",
};

pub const TEXTS_KO: Texts = Texts {
    welcome_title: "Cratos 설정에 오신 것을 환영합니다!",
    welcome_subtitle: "몇 분만에 AI 어시스턴트를 설정해 보세요.",
    welcome_steps: r#"
설정 단계:
  1. Telegram 봇 연결 (선택)
  2. Slack 봇 연결 (선택)
  3. AI 모델 선택
  4. 페르소나 선택
  5. 연결 테스트
"#,
    welcome_time: "총 소요 시간: 약 5분",

    non_interactive_title: "Cratos가 설치되었습니다!",
    non_interactive_body: r#"  대화형 설정이 필요합니다. 터미널에서 다음을 실행하세요:

    cratos init              대화형 설정 (추천)
    cratos init --lang en    English setup

  또는 .env 파일을 직접 작성할 수 있습니다:
    TELEGRAM_BOT_TOKEN, OPENAI_API_KEY 등"#,

    env_overwrite: ".env 파일이 이미 존재합니다. 덮어쓸까요?",

    telegram_title: "Telegram 봇 설정",
    telegram_desc: "Telegram 봇을 만들어야 Cratos와 대화할 수 있어요.",
    telegram_instructions: r#"
따라하기:
  1. Telegram에서 @BotFather 검색
     https://t.me/BotFather

  2. BotFather에게 /newbot 입력

  3. 봇 이름 입력 (예: 내 AI 비서)

  4. 봇 사용자명 입력 (반드시 _bot으로 끝나야 해요)

  5. 받은 토큰 복사 (예: 123456789:ABCdefGHI...)
"#,
    telegram_help_link: "도움말: https://core.telegram.org/bots#how-do-i-create-a-bot",
    telegram_prompt: "Telegram 봇 토큰을 붙여넣기 하세요:",
    telegram_skip: "Telegram 설정 건너뛰기?",
    telegram_skip_note: "(나중에 설정할 수 있어요)",

    slack_title: "Slack 봇 설정",
    slack_skip: "Slack 설정 건너뛰기?",
    slack_skip_note: "(나중에 설정할 수 있어요)",
    slack_token_prompt: "Slack 봇 토큰 입력 (xoxb-...):",
    slack_secret_prompt: "Slack signing secret 입력:",

    provider_title: "AI 모델 선택",
    provider_desc: "어떤 AI를 사용할지 선택하세요.\n처음이라면 무료 옵션을 추천해요!",
    provider_free_header: "무료 (추천)",
    provider_paid_header: "유료 (고품질)",
    provider_local_header: "로컬",
    provider_prompt: "사용할 AI를 선택하세요:",

    apikey_title: "API 키 발급",
    apikey_instructions: r#"
따라하기:
  1. 아래 링크에서 가입/로그인:
     {url}

  2. "Create Key" 또는 "API 키 생성" 버튼 클릭

  3. 생성된 키 복사
"#,
    apikey_prompt: "API 키를 붙여넣기 하세요:",
    apikey_ollama_no_key_en: "Ollama는 로컬에서 실행됩니다. API 키가 필요 없어요.",
    apikey_ollama_run_en: "Ollama가 실행 중인지 확인하세요: ollama serve",

    persona_title: "기본 페르소나 선택",
    persona_prompt: "기본 페르소나를 선택하세요:",
    persona_help: "@mention으로 페르소나 전환 가능 (예: @sindri)",

    test_title: "연결 테스트",
    test_telegram: "Telegram 연결 확인 중...",
    test_llm: "LLM 연결 확인 중...",
    test_success: "성공!",
    test_failed: "실패. 인증 정보를 확인해 주세요.",
    test_continue: "그래도 계속할까요?",

    complete_title: "설정 완료!",
    complete_saved: "설정이 .env 파일에 저장되었습니다",
    complete_summary: "요약:",
    complete_next_steps: r#"
다음 단계:
  1. Cratos 실행:        cratos serve
  2. Telegram에서 내 봇 검색
  3. 대화 시작:          /start
"#,
    complete_tips: r#"
팁:
  - 진단 실행:           cratos doctor
  - 페르소나 전환:       @sindri, @athena, @heimdall
  - 도움말 보기:         cratos --help
"#,
    complete_problems: "문제가 있으면 실행: cratos doctor",

    enabled: "활성화",
    disabled: "비활성화",
    cancel_msg: "설정이 취소되었습니다.",
};

pub fn get_texts(lang: Language) -> &'static Texts {
    match lang {
        Language::English => &TEXTS_EN,
        Language::Korean => &TEXTS_KO,
    }
}
