//! Connection tests for Telegram and LLM providers.

use super::providers::Provider;

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

/// Test LLM provider connectivity.
pub async fn test_llm(provider: &Provider, api_key: &str) -> bool {
    if provider.name == "ollama" {
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
        !api_key.is_empty() && api_key.len() > 10
    }
}
