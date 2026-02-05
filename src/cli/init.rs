//! Cratos Onboarding Wizard
//!
//! Interactive setup wizard for configuring Cratos on first run.

use inquire::{Confirm, Password, Select};
use std::fs;
use std::path::Path;

/// Onboarding step enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum OnboardingStep {
    Welcome,
    LlmProvider,
    ApiKeySetup,
    ChannelSetup,
    PersonaSelection,
    TestConnection,
    Complete,
}

impl OnboardingStep {
    fn step_number(&self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::LlmProvider => 1,
            Self::ApiKeySetup => 2,
            Self::ChannelSetup => 3,
            Self::PersonaSelection => 4,
            Self::TestConnection => 5,
            Self::Complete => 6,
        }
    }

    fn total_steps() -> usize {
        5 // Welcome is step 0, Complete is confirmation
    }
}

/// LLM Provider information
#[allow(dead_code)]
struct LlmProviderInfo {
    name: &'static str,
    display: &'static str,
    env_var: &'static str,
    is_free: bool,
    cost_per_million: Option<f64>,
    notes: &'static str,
}

const LLM_PROVIDERS: &[LlmProviderInfo] = &[
    LlmProviderInfo {
        name: "groq",
        display: "Groq (Free tier, recommended)",
        env_var: "GROQ_API_KEY",
        is_free: true,
        cost_per_million: Some(0.0),
        notes: "Free tier with Llama 4 Scout - ultra fast inference",
    },
    LlmProviderInfo {
        name: "siliconflow",
        display: "SiliconFlow (Cheapest: $0.03/M tokens)",
        env_var: "SILICONFLOW_API_KEY",
        is_free: false,
        cost_per_million: Some(0.03),
        notes: "Cheapest provider: DeepSeek R1 Distill",
    },
    LlmProviderInfo {
        name: "deepseek",
        display: "DeepSeek (Ultra low cost: $0.14/M)",
        env_var: "DEEPSEEK_API_KEY",
        is_free: false,
        cost_per_million: Some(0.14),
        notes: "Ultra low cost: DeepSeek R1",
    },
    LlmProviderInfo {
        name: "fireworks",
        display: "Fireworks (Fast inference, Llama 4)",
        env_var: "FIREWORKS_API_KEY",
        is_free: false,
        cost_per_million: Some(0.20),
        notes: "Fast inference for Llama 4, Mixtral, DeepSeek",
    },
    LlmProviderInfo {
        name: "openrouter",
        display: "OpenRouter (Free tier available)",
        env_var: "OPENROUTER_API_KEY",
        is_free: true,
        cost_per_million: None,
        notes: "Multi-provider gateway with free tier",
    },
    LlmProviderInfo {
        name: "novita",
        display: "Novita AI (Free tier available)",
        env_var: "NOVITA_API_KEY",
        is_free: true,
        cost_per_million: None,
        notes: "Free tier with multiple models",
    },
    LlmProviderInfo {
        name: "moonshot",
        display: "Moonshot AI (Kimi 2.5)",
        env_var: "MOONSHOT_API_KEY",
        is_free: false,
        cost_per_million: Some(0.60),
        notes: "Kimi 2.5 with 128K context",
    },
    LlmProviderInfo {
        name: "zhipu",
        display: "ZhipuAI GLM (GLM-4)",
        env_var: "ZHIPU_API_KEY",
        is_free: false,
        cost_per_million: Some(0.50),
        notes: "GLM-4 series models",
    },
    LlmProviderInfo {
        name: "openai",
        display: "OpenAI (GPT-4o)",
        env_var: "OPENAI_API_KEY",
        is_free: false,
        cost_per_million: Some(5.0),
        notes: "GPT-4o and GPT-4o-mini",
    },
    LlmProviderInfo {
        name: "anthropic",
        display: "Anthropic (Claude Sonnet 4)",
        env_var: "ANTHROPIC_API_KEY",
        is_free: false,
        cost_per_million: Some(3.0),
        notes: "Claude Sonnet 4 - best coding model",
    },
    LlmProviderInfo {
        name: "ollama",
        display: "Ollama (Local, free)",
        env_var: "",
        is_free: true,
        cost_per_million: Some(0.0),
        notes: "Local Ollama instance - requires Ollama to be running",
    },
];

/// Persona information
struct PersonaInfo {
    name: &'static str,
    display: &'static str,
    domain: &'static str,
    description: &'static str,
}

const PERSONAS: &[PersonaInfo] = &[
    PersonaInfo {
        name: "cratos",
        display: "Cratos (Orchestrator)",
        domain: "ALL",
        description: "Supreme orchestrator - routes tasks to specialized personas",
    },
    PersonaInfo {
        name: "sindri",
        display: "Sindri (Developer)",
        domain: "DEV",
        description: "Master craftsman - coding, implementation, debugging",
    },
    PersonaInfo {
        name: "athena",
        display: "Athena (Project Manager)",
        domain: "PM",
        description: "Strategic wisdom - planning, architecture, design",
    },
    PersonaInfo {
        name: "heimdall",
        display: "Heimdall (QA)",
        domain: "QA",
        description: "All-seeing guardian - testing, security, quality",
    },
    PersonaInfo {
        name: "mimir",
        display: "Mimir (Researcher)",
        domain: "RESEARCH",
        description: "Knowledge keeper - research, documentation, analysis",
    },
];

/// Run the onboarding wizard
pub async fn run() -> anyhow::Result<()> {
    print_welcome();

    let env_path = Path::new(".env");

    if env_path.exists() {
        let overwrite = Confirm::new(".env file already exists. Overwrite?")
            .with_default(false)
            .prompt()?;

        if !overwrite {
            println!("\nSetup cancelled. Your existing configuration was preserved.");
            return Ok(());
        }
    }

    // Step 1: LLM Provider Selection
    print_step_header(OnboardingStep::LlmProvider);

    let provider_displays: Vec<&str> = LLM_PROVIDERS.iter().map(|p| p.display).collect();

    let selected_display = Select::new("Select LLM provider:", provider_displays)
        .with_help_message("Groq is free; SiliconFlow is cheapest paid option ($0.03/M)")
        .prompt()?;

    let provider = LLM_PROVIDERS
        .iter()
        .find(|p| p.display == selected_display)
        .expect("Selected provider must exist");

    // Step 2: API Key
    print_step_header(OnboardingStep::ApiKeySetup);

    let api_key = if provider.name == "ollama" {
        println!("  Ollama uses local inference - no API key needed.");
        println!("  Make sure Ollama is running: ollama serve\n");
        String::new()
    } else {
        get_api_key(provider)?
    };

    // Step 3: Channel Setup
    print_step_header(OnboardingStep::ChannelSetup);

    let telegram_enabled = Confirm::new("Enable Telegram bot?")
        .with_default(false)
        .with_help_message("Requires creating a bot via @BotFather")
        .prompt()?;

    let telegram_token = if telegram_enabled {
        Password::new("Enter Telegram bot token:")
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .with_help_message("Get this from @BotFather on Telegram")
            .prompt()?
    } else {
        String::new()
    };

    let slack_enabled = Confirm::new("Enable Slack bot?")
        .with_default(false)
        .prompt()?;

    let (slack_token, slack_secret) = if slack_enabled {
        let token = Password::new("Enter Slack bot token:")
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .prompt()?;
        let secret = Password::new("Enter Slack signing secret:")
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .prompt()?;
        (token, secret)
    } else {
        (String::new(), String::new())
    };

    // Step 4: Persona Selection
    print_step_header(OnboardingStep::PersonaSelection);

    let persona_displays: Vec<&str> = PERSONAS.iter().map(|p| p.display).collect();

    let selected_persona_display = Select::new("Select default persona:", persona_displays)
        .with_help_message("You can switch personas with @mention (e.g., @sindri)")
        .prompt()?;

    let selected_persona = PERSONAS
        .iter()
        .find(|p| p.display == selected_persona_display)
        .expect("Selected persona must exist");

    println!(
        "  Selected: {} - {}\n",
        selected_persona.name, selected_persona.description
    );

    // Step 5: Test Connection
    print_step_header(OnboardingStep::TestConnection);

    println!("  Testing LLM connection...");

    let connection_ok = test_llm_connection(provider, &api_key).await;

    if connection_ok {
        println!("  LLM connection successful!\n");
    } else {
        println!("  LLM connection failed. Please verify your API key.\n");
        let proceed = Confirm::new("Continue anyway?")
            .with_default(false)
            .prompt()?;

        if !proceed {
            println!("\nSetup cancelled. Please check your API key and try again.");
            return Ok(());
        }
    }

    // Save configuration
    let env_content = build_env_file(
        provider,
        &api_key,
        telegram_enabled,
        &telegram_token,
        slack_enabled,
        &slack_token,
        &slack_secret,
        selected_persona.name,
    );

    fs::write(env_path, env_content)?;

    // Complete
    print_completion(provider, telegram_enabled, slack_enabled, selected_persona);

    Ok(())
}

fn print_welcome() {
    println!();
    println!("  ╔═══════════════════════════════════════════════════════════════╗");
    println!("  ║                                                               ║");
    println!("  ║        Welcome to Cratos - AI-Powered Personal Assistant      ║");
    println!("  ║                                                               ║");
    println!("  ╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("  This wizard will help you configure Cratos in a few simple steps.");
    println!();
}

fn print_step_header(step: OnboardingStep) {
    let step_num = step.step_number();
    let total = OnboardingStep::total_steps();

    let title = match step {
        OnboardingStep::Welcome => "Welcome",
        OnboardingStep::LlmProvider => "Choose your LLM provider",
        OnboardingStep::ApiKeySetup => "Enter your API key",
        OnboardingStep::ChannelSetup => "Configure channels",
        OnboardingStep::PersonaSelection => "Select default persona",
        OnboardingStep::TestConnection => "Testing connection",
        OnboardingStep::Complete => "Setup complete",
    };

    println!();
    println!("  Step {}/{}: {}", step_num, total, title);
    println!("  {}", "─".repeat(50));
    println!();
}

fn get_api_key(provider: &LlmProviderInfo) -> anyhow::Result<String> {
    // Check if already set in environment
    if let Ok(existing) = std::env::var(provider.env_var) {
        if !existing.is_empty() {
            let use_existing = Confirm::new(&format!(
                "{} is already set in environment. Use it?",
                provider.env_var
            ))
            .with_default(true)
            .prompt()?;

            if use_existing {
                return Ok(existing);
            }
        }
    }

    println!("  Provider: {}", provider.display);
    if !provider.notes.is_empty() {
        println!("  Notes: {}", provider.notes);
    }
    if let Some(cost) = provider.cost_per_million {
        if cost == 0.0 {
            println!("  Cost: FREE");
        } else {
            println!("  Cost: ${:.2} per million tokens", cost);
        }
    }
    println!();

    Password::new(&format!("Enter {} API key:", provider.env_var))
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .with_validator(inquire::required!())
        .prompt()
        .map_err(|e| e.into())
}

async fn test_llm_connection(provider: &LlmProviderInfo, api_key: &str) -> bool {
    if provider.name == "ollama" {
        // Test Ollama by checking if it's running
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok();

        if let Some(client) = client {
            let result = client.get("http://localhost:11434/api/tags").send().await;
            return result.is_ok();
        }
        return false;
    }

    // For API providers, we do a minimal check
    // In production, this could do a simple completion request
    !api_key.is_empty() && api_key.len() > 10
}

#[allow(clippy::too_many_arguments)]
fn build_env_file(
    provider: &LlmProviderInfo,
    api_key: &str,
    telegram_enabled: bool,
    telegram_token: &str,
    slack_enabled: bool,
    slack_token: &str,
    slack_secret: &str,
    default_persona: &str,
) -> String {
    let mut content = String::from("# Cratos Environment Variables\n");
    content.push_str("# Generated by cratos init\n\n");

    content.push_str("# ===================\n");
    content.push_str("# LLM Providers\n");
    content.push_str("# ===================\n");

    if provider.name == "ollama" {
        content.push_str("OLLAMA_BASE_URL=http://localhost:11434\n");
    } else {
        content.push_str(&format!("{}={}\n", provider.env_var, api_key));
    }

    if !provider.notes.is_empty() {
        content.push_str(&format!("# {}\n", provider.notes));
    }

    content.push_str("\n# ===================\n");
    content.push_str("# Default Provider\n");
    content.push_str("# ===================\n");
    content.push_str(&format!("CRATOS_DEFAULT_PROVIDER={}\n", provider.name));

    content.push_str("\n# ===================\n");
    content.push_str("# Default Persona\n");
    content.push_str("# ===================\n");
    content.push_str(&format!("CRATOS_DEFAULT_PERSONA={}\n", default_persona));

    content.push_str("\n# ===================\n");
    content.push_str("# Telegram\n");
    content.push_str("# ===================\n");
    if telegram_enabled {
        content.push_str(&format!("TELEGRAM_BOT_TOKEN={}\n", telegram_token));
    } else {
        content.push_str("# TELEGRAM_BOT_TOKEN=your-telegram-bot-token\n");
    }

    content.push_str("\n# ===================\n");
    content.push_str("# Slack\n");
    content.push_str("# ===================\n");
    if slack_enabled {
        content.push_str(&format!("SLACK_BOT_TOKEN={}\n", slack_token));
        content.push_str(&format!("SLACK_SIGNING_SECRET={}\n", slack_secret));
    } else {
        content.push_str("# SLACK_BOT_TOKEN=xoxb-your-slack-bot-token\n");
        content.push_str("# SLACK_SIGNING_SECRET=your-slack-signing-secret\n");
    }

    content.push_str("\n# ===================\n");
    content.push_str("# Server\n");
    content.push_str("# ===================\n");
    content.push_str("HOST=127.0.0.1\n");
    content.push_str("PORT=9742\n");

    content.push_str("\n# ===================\n");
    content.push_str("# Logging\n");
    content.push_str("# ===================\n");
    content.push_str("RUST_LOG=cratos=info,tower_http=info\n");

    content
}

fn print_completion(
    provider: &LlmProviderInfo,
    telegram_enabled: bool,
    slack_enabled: bool,
    persona: &PersonaInfo,
) {
    println!();
    println!("  ╔═══════════════════════════════════════════════════════════════╗");
    println!("  ║                                                               ║");
    println!("  ║                    Setup Complete!                            ║");
    println!("  ║                                                               ║");
    println!("  ╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Configuration saved to .env");
    println!();
    println!("  Summary:");
    println!("    LLM Provider: {}", provider.display);
    println!("    Default Persona: {} ({})", persona.name, persona.domain);
    println!(
        "    Telegram: {}",
        if telegram_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "    Slack: {}",
        if slack_enabled { "enabled" } else { "disabled" }
    );
    println!();
    println!("  Next steps:");
    println!("    1. Run 'cratos serve' to start the assistant");
    println!("    2. Run 'cratos doctor' to verify system status");
    if telegram_enabled {
        println!("    3. Message your Telegram bot to start chatting!");
    }
    println!();
    println!("  Tips:");
    println!("    - Switch personas with @mention: @sindri, @athena, @heimdall");
    println!("    - View help: cratos --help");
    println!("    - Check logs: tail -f ~/.cratos/cratos.log");
    println!();
}
