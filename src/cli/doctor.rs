use std::path::Path;
use std::process::Command;

fn has_valid_key(content: &str, key_name: &str, valid_prefix: &str) -> bool {
    content
        .lines()
        .find(|l| l.starts_with(&format!("{}=", key_name)))
        .map(|l| l.trim_start_matches(&format!("{}=", key_name)))
        .map(|v| {
            !v.is_empty()
                && !v.contains("your")
                && (valid_prefix.is_empty() || v.starts_with(valid_prefix))
        })
        .unwrap_or(false)
}

pub async fn run() -> anyhow::Result<()> {
    println!("🏥 Cratos Doctor\n");

    let mut all_ok = true;

    all_ok &= check_rust_version().await;
    all_ok &= check_env_file().await;
    all_ok &= check_cli_auth().await;
    all_ok &= check_data_dir().await;
    all_ok &= check_llm_config().await;
    check_container_runtime().await;
    check_redis().await;

    println!();
    if all_ok {
        println!("✅ All checks passed! Ready to run Cratos.");
    } else {
        println!("⚠️  Some checks failed. Please fix the issues above.");
        std::process::exit(1);
    }

    Ok(())
}

async fn check_rust_version() -> bool {
    print!("Checking Rust version... ");

    match Command::new("rustc").arg("--version").output() {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout);
            let version_str = version.trim();

            if version_str.contains("1.80")
                || version_str.contains("1.8")
                || version_str.contains("1.9")
            {
                println!("✅ {}", version_str);
                true
            } else {
                println!("⚠️  {} (1.80+ recommended)", version_str);
                true
            }
        }
        Err(_) => {
            println!("❌ Rust not found. Please install Rust: https://rustup.rs");
            false
        }
    }
}

async fn check_env_file() -> bool {
    print!("Checking .env file... ");

    if Path::new(".env").exists() {
        println!("✅ Found");

        let env_content = std::fs::read_to_string(".env").unwrap_or_default();

        let has_llm_key = has_valid_key(&env_content, "GROQ_API_KEY", "gsk_")
            || has_valid_key(&env_content, "GOOGLE_API_KEY", "")
            || has_valid_key(&env_content, "SILICONFLOW_API_KEY", "")
            || has_valid_key(&env_content, "FIREWORKS_API_KEY", "fw_")
            || has_valid_key(&env_content, "OPENROUTER_API_KEY", "sk-or-")
            || has_valid_key(&env_content, "NOVITA_API_KEY", "")
            || has_valid_key(&env_content, "DEEPSEEK_API_KEY", "sk-")
            || has_valid_key(&env_content, "MOONSHOT_API_KEY", "")
            || has_valid_key(&env_content, "ZHIPU_API_KEY", "")
            || has_valid_key(&env_content, "OPENAI_API_KEY", "sk-")
            || has_valid_key(&env_content, "ANTHROPIC_API_KEY", "sk-ant-")
            || env_content.contains("OLLAMA_BASE_URL=");

        if has_llm_key {
            println!("  ✅ LLM API key configured");
        } else {
            println!("  ⚠️  No LLM API key found. Run 'cratos init' to configure.");
            return false;
        }

        true
    } else {
        println!("❌ Not found");
        println!("  Run 'cratos init' to create .env file");
        false
    }
}

async fn check_data_dir() -> bool {
    print!("Checking data directory... ");

    let data_dir = dirs::home_dir()
        .map(|h| h.join(".cratos"))
        .unwrap_or_else(|| Path::new(".cratos").to_path_buf());

    if data_dir.exists() {
        println!("✅ {}", data_dir.display());

        let db_path = data_dir.join("cratos.db");
        if db_path.exists() {
            println!("  ✅ Database exists");
        } else {
            println!("  ℹ️  Database will be created on first run");
        }

        true
    } else {
        println!("ℹ️  Will create {}", data_dir.display());
        true
    }
}

async fn check_llm_config() -> bool {
    print!("Checking LLM connectivity... ");

    let env_content = std::fs::read_to_string(".env").unwrap_or_default();

    if env_content.contains("OPENROUTER_API_KEY=")
        && !env_content.contains("OPENROUTER_API_KEY=sk-or-your")
    {
        println!("ℹ️  OpenRouter configured (connectivity test skipped)");
        return true;
    }

    if env_content.contains("OLLAMA_BASE_URL=") {
        let client = reqwest::Client::new();
        match client
            .get("http://localhost:11434/api/tags")
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(_) => {
                println!("✅ Ollama is running");
                true
            }
            Err(_) => {
                println!("⚠️  Ollama not running on localhost:11434");
                println!("  Start Ollama with: ollama serve");
                false
            }
        }
    } else {
        println!("ℹ️  Skipped (external API)");
        true
    }
}

async fn check_cli_auth() -> bool {
    use cratos_llm::cli_auth;

    println!("Checking CLI auth tokens...");

    let gemini_status = cli_auth::check_gemini_cli_status();
    match gemini_status {
        cli_auth::GeminiCliStatus::Valid => {
            println!("  ✅ Gemini CLI: {} (Antigravity Pro)", gemini_status);
        }
        cli_auth::GeminiCliStatus::Expired => {
            println!("  ⚠️  Gemini CLI: {}", gemini_status);
        }
        cli_auth::GeminiCliStatus::NotFound => {
            println!("  ℹ️  Gemini CLI: not configured (optional)");
        }
        _ => {
            println!("  ⚠️  Gemini CLI: {}", gemini_status);
        }
    }

    let codex_status = cli_auth::check_codex_cli_status();
    match codex_status {
        cli_auth::CodexCliStatus::Valid => {
            println!("  ✅ Codex CLI: {} (ChatGPT Pro/Plus)", codex_status);
        }
        cli_auth::CodexCliStatus::NotFound => {
            println!("  ℹ️  Codex CLI: not configured (optional)");
        }
        _ => {
            println!("  ⚠️  Codex CLI: {}", codex_status);
        }
    }

    // CLI auth is optional, never fails the check
    true
}

async fn check_container_runtime() -> bool {
    print!("Checking container runtime... ");

    // Check Apple Container first (macOS 26+)
    #[cfg(target_os = "macos")]
    {
        if std::env::consts::ARCH == "aarch64" {
            if let Ok(output) = Command::new("container").arg("--version").output() {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    println!("✅ Apple Container {}", version.trim());
                    return true;
                }
            }
        }
    }

    // Check Docker
    match Command::new("docker").arg("info").output() {
        Ok(output) => {
            if output.status.success() {
                println!("✅ Docker available");
                true
            } else {
                println!("⚠️  Docker installed but not running");
                println!("  Start Docker Desktop or run: docker daemon");
                true
            }
        }
        Err(_) => {
            println!("ℹ️  No container runtime (sandboxing disabled)");
            println!("  Install Docker for enhanced security isolation");
            true // Not required, just informational
        }
    }
}

async fn check_redis() -> bool {
    print!("Checking Redis... ");

    let env_content = std::fs::read_to_string(".env").unwrap_or_default();

    if !env_content.contains("REDIS_URL=") || env_content.contains("# REDIS_URL=") {
        println!("ℹ️  Not configured (optional)");
        return true;
    }

    let redis_url = env_content
        .lines()
        .find(|l| l.starts_with("REDIS_URL="))
        .map(|l| l.trim_start_matches("REDIS_URL="))
        .unwrap_or("redis://localhost:6379");

    match redis::Client::open(redis_url) {
        Ok(client) => match client.get_multiplexed_async_connection().await {
            Ok(mut conn) => match redis::cmd("PING").query_async::<String>(&mut conn).await {
                Ok(_) => {
                    println!("✅ Connected");
                    true
                }
                Err(e) => {
                    println!("⚠️  Ping failed: {}", e);
                    false
                }
            },
            Err(e) => {
                println!("⚠️  Connection failed: {}", e);
                false
            }
        },
        Err(e) => {
            println!("⚠️  Invalid URL: {}", e);
            false
        }
    }
}
