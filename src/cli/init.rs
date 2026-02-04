use inquire::{Confirm, Password, Select};
use std::fs;
use std::path::Path;

const ENV_EXAMPLE: &str = include_str!("../../.env.example");

pub async fn run() -> anyhow::Result<()> {
    println!("🚀 Cratos Setup Wizard\n");

    let env_path = Path::new(".env");
    
    if env_path.exists() {
        let overwrite = Confirm::new(".env file already exists. Overwrite?")
            .with_default(false)
            .prompt()?;
        
        if !overwrite {
            println!("Setup cancelled.");
            return Ok(());
        }
    }

    let llm_provider = Select::new("Select LLM provider:", vec![
        "Groq (Free tier, recommended)",
        "OpenRouter (Free tier available)",
        "Novita AI (Free tier available)",
        "DeepSeek (Ultra low cost)",
        "OpenAI (GPT-4)",
        "Anthropic (Claude)",
        "Ollama (Local, free)",
    ])
    .with_help_message("Groq is recommended - free tier with fast inference")
    .prompt()?;

    let api_key = match llm_provider {
        "Groq (Free tier, recommended)" => get_api_key("Groq"),
        "OpenRouter (Free tier available)" => get_api_key("OpenRouter"),
        "Novita AI (Free tier available)" => get_api_key("Novita"),
        "DeepSeek (Ultra low cost)" => get_api_key("DeepSeek"),
        "OpenAI (GPT-4)" => get_api_key("OpenAI"),
        "Anthropic (Claude)" => get_api_key("Anthropic"),
        "Ollama (Local, free)" => Ok(String::new()),
        _ => Ok(String::new()),
    }?;

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

    let env_content = build_env_file(
        llm_provider,
        &api_key,
        telegram_enabled,
        &telegram_token,
        slack_enabled,
        &slack_token,
        &slack_secret,
    );

    fs::write(env_path, env_content)?;

    println!("\n✅ Configuration saved to .env");
    println!("\nNext steps:");
    println!("  1. Review .env file if needed");
    println!("  2. Run 'cargo run' to start Cratos");
    println!("  3. Run 'cratos doctor' to verify setup");

    Ok(())
}

fn get_api_key(provider: &str) -> anyhow::Result<String> {
    Password::new(&format!("Enter {} API key:", provider))
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .with_validator(inquire::required!())
        .prompt()
        .map_err(|e| e.into())
}

fn build_env_file(
    provider: &str,
    api_key: &str,
    telegram_enabled: bool,
    telegram_token: &str,
    slack_enabled: bool,
    slack_token: &str,
    slack_secret: &str,
) -> String {
    let mut content = String::from("# Cratos Environment Variables\n\n");

    content.push_str("# ===================\n");
    content.push_str("# LLM Providers\n");
    content.push_str("# ===================\n");

    match provider {
        "Groq (Free tier, recommended)" => {
            content.push_str(&format!("GROQ_API_KEY={}\n", api_key));
        }
        "OpenRouter (Free tier available)" => {
            content.push_str(&format!("OPENROUTER_API_KEY={}\n", api_key));
        }
        "Novita AI (Free tier available)" => {
            content.push_str(&format!("NOVITA_API_KEY={}\n", api_key));
        }
        "DeepSeek (Ultra low cost)" => {
            content.push_str(&format!("DEEPSEEK_API_KEY={}\n", api_key));
        }
        "OpenAI (GPT-4)" => {
            content.push_str(&format!("OPENAI_API_KEY={}\n", api_key));
        }
        "Anthropic (Claude)" => {
            content.push_str(&format!("ANTHROPIC_API_KEY={}\n", api_key));
        }
        "Ollama (Local, free)" => {
            content.push_str("OLLAMA_BASE_URL=http://localhost:11434\n");
        }
        _ => {}
    }

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
