//! Connection tests for Telegram and LLM providers.

use super::providers::Provider;

/// Ollama-specific status for guiding the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OllamaStatus {
    /// Server responding on localhost:11434
    Running,
    /// Binary found but server not responding
    NotRunning,
    /// `which ollama` failed — not installed
    NotInstalled,
}

/// Check Ollama installation and server status.
pub async fn check_ollama() -> OllamaStatus {
    // 1. Check if the binary exists
    let installed = std::process::Command::new("which")
        .arg("ollama")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !installed {
        return OllamaStatus::NotInstalled;
    }

    // 2. Check if the server is responding
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return OllamaStatus::NotRunning,
    };

    match client.get("http://localhost:11434/api/tags").send().await {
        Ok(resp) if resp.status().is_success() => OllamaStatus::Running,
        _ => OllamaStatus::NotRunning,
    }
}

/// Test Telegram token by calling the getMe API.
pub async fn test_telegram(token: &str) -> bool {
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

/// Test LLM provider connectivity with an actual API call.
pub async fn test_llm(provider: &Provider, api_key: &str) -> bool {
    if provider.name == "ollama" {
        return test_ollama_model().await;
    }

    // For providers that support CLI auth, try OAuth if api_key is empty
    if api_key.is_empty() {
        return match provider.name {
            "google" => test_gemini_oauth().await,
            "openai" => test_openai_codex_auth().await,
            _ => false,
        };
    }

    if api_key.len() < 10 {
        return false;
    }

    // Build a real test request to verify the API key works and the model responds
    let (base_url, model, auth_header) = match provider.name {
        "groq" => (
            "https://api.groq.com/openai/v1/chat/completions",
            "llama-3.3-70b-versatile",
            format!("Bearer {}", api_key),
        ),
        "openrouter" => (
            "https://openrouter.ai/api/v1/chat/completions",
            "google/gemini-flash-1.5",
            format!("Bearer {}", api_key),
        ),
        "google" => {
            // Gemini uses a different API format
            return test_gemini(api_key).await;
        }
        "novita" => (
            "https://api.novita.ai/v3/openai/chat/completions",
            "qwen/qwen2.5-7b-instruct",
            format!("Bearer {}", api_key),
        ),
        "deepseek" => (
            "https://api.deepseek.com/v1/chat/completions",
            "deepseek-chat",
            format!("Bearer {}", api_key),
        ),
        "openai" => (
            "https://api.openai.com/v1/chat/completions",
            "gpt-4o-mini",
            format!("Bearer {}", api_key),
        ),
        "anthropic" => {
            return test_anthropic(api_key).await;
        }
        _ => return true, // Unknown provider, skip real test
    };

    test_openai_compatible(base_url, model, &auth_header).await
}

/// Test with OpenAI-compatible API (Groq, OpenRouter, DeepSeek, Novita, OpenAI)
async fn test_openai_compatible(url: &str, model: &str, auth: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "Say OK"}],
        "max_tokens": 5,
        "temperature": 0
    });

    match client
        .post(url)
        .header("Authorization", auth)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Test Anthropic API (different format from OpenAI)
async fn test_anthropic(api_key: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let body = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 5,
        "messages": [{"role": "user", "content": "Say OK"}]
    });

    match client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Test Google Gemini API
async fn test_gemini(api_key: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
        api_key
    );
    let body = serde_json::json!({
        "contents": [{"parts": [{"text": "Say OK"}]}],
        "generationConfig": {"maxOutputTokens": 5}
    });

    match client.post(&url).json(&body).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Test a Google OAuth Bearer token via Code Assist endpoint.
///
/// The standard `generativelanguage.googleapis.com` doesn't accept OAuth Bearer
/// tokens. Gemini CLI uses Code Assist (`cloudcode-pa.googleapis.com`), so we test there.
pub async fn test_google_oauth_token(access_token: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("Failed to build HTTP client: {}", e);
            return false;
        }
    };

    let body = serde_json::json!({
        "metadata": {
            "ideType": "IDE_UNSPECIFIED",
            "platform": "PLATFORM_UNSPECIFIED",
            "pluginType": "GEMINI",
        }
    });

    match client
        .post("https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist")
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                tracing::debug!("Google OAuth test failed (HTTP {}): {}", status, body);
            }
            status.is_success()
        }
        Err(e) => {
            tracing::debug!("Google OAuth test network error: {}", e);
            false
        }
    }
}

/// Test Google Gemini with OAuth token (Cratos OAuth → Gemini CLI fallback).
async fn test_gemini_oauth() -> bool {
    if let Some(tokens) = cratos_llm::cli_auth::read_cratos_google_oauth() {
        if test_google_oauth_token(&tokens.access_token).await {
            return true;
        }
        if let Some(ref rt) = tokens.refresh_token {
            let config = cratos_llm::oauth_config::google_oauth_config();
            if let Ok(refreshed) = cratos_llm::oauth::refresh_token(&config, rt).await {
                let _ = cratos_llm::oauth::save_tokens(&config.token_file, &refreshed);
                if test_google_oauth_token(&refreshed.access_token).await {
                    return true;
                }
            }
        }
    }

    let creds = match cratos_llm::cli_auth::read_gemini_oauth() {
        Some(c) => c,
        None => return false,
    };

    if test_google_oauth_token(&creds.access_token).await {
        return true;
    }

    match cratos_llm::cli_auth::refresh_gemini_token().await {
        Ok(refreshed) => test_google_oauth_token(&refreshed.access_token).await,
        Err(_) => false,
    }
}

/// Test OpenAI with OAuth token (Cratos OAuth → Codex CLI fallback).
async fn test_openai_codex_auth() -> bool {
    if let Some(tokens) = cratos_llm::cli_auth::read_cratos_openai_oauth() {
        let auth = format!("Bearer {}", tokens.access_token);
        if test_openai_compatible("https://api.openai.com/v1/chat/completions", "gpt-4o-mini", &auth).await {
            return true;
        }
        if let Some(ref rt) = tokens.refresh_token {
            let config = cratos_llm::oauth_config::openai_oauth_config();
            if let Ok(refreshed) = cratos_llm::oauth::refresh_token(&config, rt).await {
                let _ = cratos_llm::oauth::save_tokens(&config.token_file, &refreshed);
                let auth = format!("Bearer {}", refreshed.access_token);
                if test_openai_compatible("https://api.openai.com/v1/chat/completions", "gpt-4o-mini", &auth).await {
                    return true;
                }
            }
        }
    }

    let creds = match cratos_llm::cli_auth::read_codex_auth() {
        Some(c) => c,
        None => return false,
    };

    let auth = format!("Bearer {}", creds.tokens.access_token);
    test_openai_compatible("https://api.openai.com/v1/chat/completions", "gpt-4o-mini", &auth).await
}

/// Test Ollama with an actual model inference
async fn test_ollama_model() -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    // First check if server is running
    let tags = match client.get("http://localhost:11434/api/tags").send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return false,
    };

    // Check if any model is available
    let body: serde_json::Value = match tags.json().await {
        Ok(v) => v,
        Err(_) => return false,
    };

    let has_models = body["models"]
        .as_array()
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    if !has_models {
        return false;
    }

    // Try a quick inference
    let test_body = serde_json::json!({
        "model": "qwen2.5:7b",
        "messages": [{"role": "user", "content": "Say OK"}],
        "stream": false,
        "options": {"num_predict": 5}
    });

    match client
        .post("http://localhost:11434/api/chat")
        .json(&test_body)
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Recommended Ollama models for tool-calling (ordered by preference).
pub const OLLAMA_RECOMMENDED_MODELS: &[OllamaModel] = &[
    OllamaModel {
        name: "qwen2.5:7b",
        size: "4.7 GB",
        desc_en: "Best for tool calling & multilingual (recommended)",
        desc_ko: "도구 호출 & 다국어 최적 (추천)",
    },
    OllamaModel {
        name: "llama3.1:8b",
        size: "4.7 GB",
        desc_en: "Solid general purpose",
        desc_ko: "범용 안정적",
    },
    OllamaModel {
        name: "gemma2:9b",
        size: "5.4 GB",
        desc_en: "High quality, larger",
        desc_ko: "고품질, 큰 모델",
    },
    OllamaModel {
        name: "mistral:7b",
        size: "4.1 GB",
        desc_en: "Fast, lightweight",
        desc_ko: "빠르고 가벼움",
    },
];

pub struct OllamaModel {
    pub name: &'static str,
    pub size: &'static str,
    pub desc_en: &'static str,
    pub desc_ko: &'static str,
}

/// List installed Ollama models. Returns model names (e.g. ["llama3.2:latest", "qwen2.5:7b"]).
pub async fn list_ollama_models() -> Vec<String> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let resp = match client.get("http://localhost:11434/api/tags").send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return Vec::new(),
    };

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    body["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Check if any of the recommended models (or a reasonably sized model) is installed.
pub fn has_suitable_model(installed: &[String]) -> Option<String> {
    // Check recommended models first
    for rec in OLLAMA_RECOMMENDED_MODELS {
        for inst in installed {
            // Match "qwen2.5:7b" against "qwen2.5:7b" or "qwen2.5:7b-instruct" etc.
            if inst.starts_with(rec.name) || inst.starts_with(rec.name.split(':').next().unwrap_or("")) && inst.contains(rec.name.split(':').nth(1).unwrap_or("")) {
                return Some(inst.clone());
            }
        }
    }
    // Any 7B+ model is okay
    for inst in installed {
        // Skip tiny models (1b, 3b)
        let lower = inst.to_lowercase();
        if lower.contains("1b") || lower.contains("3b") || lower.contains("0.5b") {
            continue;
        }
        return Some(inst.clone());
    }
    None
}

/// Pull an Ollama model, printing progress to stdout.
pub async fn pull_ollama_model(model: &str) -> bool {
    use std::process::Stdio;

    println!();
    println!("  Downloading {}...", model);
    println!("  (This may take a few minutes depending on your connection)");
    println!();

    let result = tokio::process::Command::new("ollama")
        .args(["pull", model])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await;

    match result {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}
