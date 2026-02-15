//! Unified interactive setup for Cratos.
//!
//! Replaces both `init.rs` and `wizard.rs` with a single flow:
//!   1. Non-interactive detection → print instructions and exit
//!   2. Language detection (or --lang flag)
//!   3. Welcome
//!   4. .env overwrite check
//!   5. Telegram setup (skippable)
//!   6. Slack setup (skippable)
//!   7. LLM provider selection (FREE / PAID / LOCAL)
//!   8. API key input
//!   9. Persona selection
//!  10. Connection tests (Telegram getMe + LLM)
//!  11. Save .env
//!  12. Summary

mod env_builder;
mod i18n;
mod oauth_server;
mod personas;
mod prompts;
mod providers;
mod testing;

use i18n::{detect_language, get_texts, Language};
use personas::PERSONAS;
use providers::{ProviderCategory, PROVIDERS};
use std::io::IsTerminal;
use std::path::Path;

/// Print a styled header line.
fn print_header(text: &str) {
    let line = "━".repeat(60);
    println!();
    println!("  {}", line);
    println!("  {}", text);
    println!("  {}", line);
    println!();
}

/// Print a styled box.
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
    println!();
}

/// Detect if running in a headless environment (SSH, VPS, etc.)
fn is_headless() -> bool {
    // SSH session
    if std::env::var("SSH_CLIENT").is_ok() || std::env::var("SSH_TTY").is_ok() {
        return true;
    }

    // Linux without display server
    #[cfg(target_os = "linux")]
    {
        if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
            return true;
        }
    }

    false
}

/// Run the unified setup wizard. Called by `cratos init` and `cratos serve` (when no .env).
pub async fn run(lang_override: Option<&str>) -> anyhow::Result<()> {
    // Resolve language
    let lang = match lang_override {
        Some(code) => Language::from_code(code),
        None => detect_language(),
    };
    let t = get_texts(lang);

    // ── 1. Non-interactive detection ──
    if !std::io::stdin().is_terminal() {
        print_header(t.non_interactive_title);
        println!("{}", t.non_interactive_body);
        println!();
        return Ok(());
    }

    // ── 2-3. Welcome ──
    print_box(t.welcome_title, t.welcome_subtitle);
    println!("{}", t.welcome_steps);
    println!("  {}\n", t.welcome_time);

    // ── 4. .env overwrite check ──
    let env_path = Path::new(super::ENV_FILE_PATH);
    if env_path.exists() {
        let overwrite = prompts::confirm(t.env_overwrite, false, None)?;
        if !overwrite {
            println!("\n  {}", t.cancel_msg);
            return Ok(());
        }
    }

    // ── 5. Telegram ──
    print_header(t.telegram_title);
    println!("  {}", t.telegram_desc);
    println!("{}", t.telegram_instructions);
    println!("  {}", t.telegram_help_link);
    println!();

    let skip_telegram = prompts::confirm(t.telegram_skip, true, Some(t.telegram_skip_note))?;

    let telegram_token = if skip_telegram {
        String::new()
    } else {
        prompts::password(t.telegram_prompt)?
    };

    // ── 6. Slack ──
    print_header(t.slack_title);
    println!("  {}", t.slack_desc);
    println!("{}", t.slack_instructions);
    println!("  {}", t.slack_help_link);
    println!();

    let skip_slack = prompts::confirm(t.slack_skip, true, Some(t.slack_skip_note))?;

    let (slack_token, slack_secret) = if skip_slack {
        (String::new(), String::new())
    } else {
        let token = prompts::password(t.slack_token_prompt)?;
        let secret = prompts::password(t.slack_secret_prompt)?;
        (token, secret)
    };

    // ── 7. LLM Provider ──
    print_header(t.provider_title);
    println!("  {}", t.provider_desc);
    println!();

    let mut options: Vec<String> = Vec::new();
    let mut provider_indices: Vec<usize> = Vec::new();

    // FREE
    options.push(format!("── {} ──", t.provider_free_header));
    provider_indices.push(usize::MAX);
    for (idx, p) in PROVIDERS.iter().enumerate() {
        if p.category == ProviderCategory::Free {
            options.push(format!("  {} ({})", p.display(lang), p.cost(lang)));
            provider_indices.push(idx);
        }
    }

    // PAID
    options.push(format!("── {} ──", t.provider_paid_header));
    provider_indices.push(usize::MAX);
    for (idx, p) in PROVIDERS.iter().enumerate() {
        if p.category == ProviderCategory::Paid {
            options.push(format!("  {} ({})", p.display(lang), p.cost(lang)));
            provider_indices.push(idx);
        }
    }

    // LOCAL
    options.push(format!("── {} ──", t.provider_local_header));
    provider_indices.push(usize::MAX);
    for (idx, p) in PROVIDERS.iter().enumerate() {
        if p.category == ProviderCategory::Local {
            options.push(format!("  {} ({})", p.display(lang), p.cost(lang)));
            provider_indices.push(idx);
        }
    }

    let selected = prompts::select(t.provider_prompt, &options)?;

    let provider_idx = provider_indices
        .iter()
        .enumerate()
        .find_map(|(list_pos, &prov_idx)| {
            if prov_idx != usize::MAX && options[list_pos] == selected {
                Some(prov_idx)
            } else {
                None
            }
        })
        .unwrap_or(0);

    let provider = &PROVIDERS[provider_idx];

    // ── 8. API Key ──
    let api_key = if provider.name == "ollama" {
        println!();
        println!("  {}", t.apikey_ollama_no_key_en);

        // Check Ollama installation and status
        let status = testing::check_ollama().await;
        match status {
            testing::OllamaStatus::NotInstalled => {
                println!("{}", t.ollama_install_guide);
                let proceed = prompts::confirm(t.test_continue, false, None)?;
                if !proceed {
                    println!("\n  {}", t.cancel_msg);
                    return Ok(());
                }
            }
            testing::OllamaStatus::NotRunning => {
                println!("{}", t.ollama_not_running);
                let proceed = prompts::confirm(t.test_continue, true, None)?;
                if !proceed {
                    println!("\n  {}", t.cancel_msg);
                    return Ok(());
                }
            }
            testing::OllamaStatus::Running => {
                // Check installed models and auto-pull if needed
                println!("  {}", t.ollama_checking_models);
                let installed = testing::list_ollama_models().await;
                if let Some(model) = testing::has_suitable_model(&installed) {
                    println!("  {}", t.ollama_suitable_model_found.replace("{}", &model));
                } else {
                    println!("{}", t.ollama_no_suitable_model);

                    let skip = prompts::confirm(t.ollama_skip_pull, false, None)?;
                    if !skip {
                        // Build model selection list
                        let model_options: Vec<String> = testing::OLLAMA_RECOMMENDED_MODELS
                            .iter()
                            .map(|m| {
                                let desc = match lang {
                                    i18n::Language::Korean => m.desc_ko,
                                    i18n::Language::English => m.desc_en,
                                };
                                format!("{} [{}] — {}", m.name, m.size, desc)
                            })
                            .collect();

                        let selected = prompts::select(t.ollama_select_model, &model_options)?;
                        let model_name = selected.split_whitespace().next().unwrap_or("qwen2.5:7b");

                        if testing::pull_ollama_model(model_name).await {
                            println!("\n  {}", t.ollama_pull_success);
                        } else {
                            println!("\n  {}", t.ollama_pull_failed);
                        }
                    }
                }
            }
        }
        String::new()
    } else {
        // Check if already set in environment
        let existing = std::env::var(provider.env_var).unwrap_or_default();
        if !existing.is_empty() {
            let use_existing = prompts::confirm(
                &format!("{} is already set. Use it?", provider.env_var),
                true,
                None,
            )?;
            if use_existing {
                existing
            } else {
                prompt_api_key(provider, lang, t)?
            }
        } else {
            match provider.name {
                "google" => resolve_google_auth(provider, lang, t).await?,
                "google_pro" => resolve_google_pro_auth(provider, lang, t).await?,
                "openai" => resolve_openai_auth(provider, lang, t).await?,
                _ => prompt_api_key(provider, lang, t)?,
            }
        }
    };

    // ── 9. Persona ──
    print_header(t.persona_title);
    println!("  {}\n", t.persona_help);

    let persona_options: Vec<String> = PERSONAS
        .iter()
        .map(|p| format!("{} — {}", p.display(lang), p.description(lang)))
        .collect();

    let selected_persona = prompts::select(t.persona_prompt, &persona_options)?;

    let persona = PERSONAS
        .iter()
        .find(|p| selected_persona.starts_with(p.display(lang)))
        .unwrap_or(&PERSONAS[0]);

    println!("\n  Selected: {} ({})\n", persona.name, persona.domain);

    // ── 10. Connection tests ──
    print_header(t.test_title);

    let mut all_ok = true;

    if !telegram_token.is_empty() {
        print!("  {} ", t.test_telegram);
        if testing::test_telegram(&telegram_token).await {
            println!("{}", t.test_success);
        } else {
            println!("{}", t.test_failed);
            all_ok = false;
        }
    }

    // CLI auth detected if api_key is empty but provider is not ollama
    let has_cli_auth = api_key.is_empty()
        && provider.name != "ollama"
        && (provider.name == "google" || provider.name == "openai");

    if !api_key.is_empty() || provider.name == "ollama" || has_cli_auth {
        print!("  {} ", t.test_llm);
        if testing::test_llm(provider, &api_key).await {
            println!("{}", t.test_success);
        } else if provider.name == "ollama" {
            let status = testing::check_ollama().await;
            match status {
                testing::OllamaStatus::NotInstalled => {
                    println!("{}", t.ollama_test_failed_not_installed);
                }
                testing::OllamaStatus::NotRunning => {
                    println!("{}", t.ollama_test_failed_not_running);
                }
                testing::OllamaStatus::Running => {
                    println!("{}", t.test_failed);
                }
            }
            all_ok = false;
        } else {
            println!("{}", t.test_failed);
            all_ok = false;
        }
    }

    if !all_ok {
        println!();
        let proceed = prompts::confirm(t.test_continue, false, None)?;
        if !proceed {
            println!("\n  {}", t.cancel_msg);
            return Ok(());
        }
    }

    // ── 11. Save .env ──
    let env_content = env_builder::build_env(
        provider,
        &api_key,
        &telegram_token,
        &slack_token,
        &slack_secret,
        persona.name,
    );
    std::fs::write(env_path, env_content)?;

    // ── 12. Summary ──
    print_box(t.complete_title, "");
    println!("  {}", t.complete_saved);
    println!();
    println!("  {}", t.complete_summary);
    println!("    LLM: {}", provider.display(lang));
    println!("    Persona: {} ({})", persona.name, persona.domain);
    println!(
        "    Telegram: {}",
        if telegram_token.is_empty() {
            t.disabled
        } else {
            t.enabled
        }
    );
    println!(
        "    Slack: {}",
        if slack_token.is_empty() {
            t.disabled
        } else {
            t.enabled
        }
    );
    println!("{}", t.complete_next_steps);
    println!("{}", t.complete_tips);
    println!("  {}", t.complete_problems);
    println!();

    Ok(())
}

/// Helper: show instructions and prompt for an API key.
fn prompt_api_key(
    provider: &providers::Provider,
    _lang: Language,
    t: &i18n::Texts,
) -> anyhow::Result<String> {
    print_header(&format!("{} — {}", t.apikey_title, provider.name));

    let instructions = t.apikey_instructions.replace("{url}", provider.signup_url);
    println!("{}", instructions);
    println!();

    prompts::password_required(t.apikey_prompt)
}

/// Resolve Google auth: Cratos OAuth → browser login → API key fallback.
async fn resolve_google_auth(
    provider: &providers::Provider,
    lang: Language,
    t: &i18n::Texts,
) -> anyhow::Result<String> {
    use cratos_llm::cli_auth::*;

    // google_oauth_config() handles credential resolution internally:
    // 1. CRATOS_GOOGLE_CLIENT_ID/SECRET env vars (user override)
    // 2. Gemini CLI credentials (auto-detected)
    // 3. Embedded gcloud SDK credentials (fallback)
    // No need to pre-set env vars here.

    // 1. Check existing Cratos OAuth token
    match check_cratos_google_oauth_status() {
        CratosOAuthStatus::Valid => {
            println!("\n  {} {}", t.oauth_detected, t.oauth_token_valid);

            // Ask if user wants to reuse existing token
            if prompts::confirm(t.oauth_reuse_prompt, true, None)? {
                return Ok(String::new());
            }
        }
        CratosOAuthStatus::Expired => {
            println!("\n  {} {}", t.oauth_detected, t.oauth_token_expired);

            if prompts::confirm(t.oauth_reuse_prompt, true, None)? {
                println!("  {}", t.oauth_refreshing);

                if let Some(tokens) = read_cratos_google_oauth() {
                    if let Some(ref rt) = tokens.refresh_token {
                        let config = cratos_llm::oauth_config::google_oauth_config();
                        match oauth_server::refresh_and_save(&config, rt).await {
                            Ok(_) => {
                                println!("  {}", t.oauth_refresh_success);
                                return Ok(String::new());
                            }
                            Err(e) => {
                                tracing::warn!("Google OAuth refresh failed: {}", e);
                            }
                        }
                    }
                }
                println!("  {}", t.oauth_refresh_failed);
            }
        }
        CratosOAuthStatus::NotFound => {}
    }

    // 2. Check for gcloud CLI
    if let Ok(_token) = get_gcloud_access_token().await {
        println!();
        print_header(t.gcloud_detected_title);
        println!("  {}", t.gcloud_detected_desc);

        if prompts::confirm(t.gcloud_use_prompt, true, None)? {
            // Verify token is valid (non-empty is checked by get_gcloud_access_token)
            // We use the token to check validity implicitly, but get_gcloud_access_token() essentially does that by running the command.
            println!("  {}", t.gcloud_success);
            return Ok(String::new());
        }
    }

    // 3. Browser OAuth login
    let headless = is_headless();

    if headless {
        println!("\n  {}", t.oauth_remote_detected);
    }

    println!("\n  {}", t.oauth_browser_login_google);
    println!("  {}", t.oauth_restricted_client_hint);

    if prompts::confirm(t.oauth_browser_login_prompt, true, None)? {
        println!("  {}", t.oauth_starting);
        // Note: oauth_waiting is printed inside run_oauth_flow for normal mode

        let config = cratos_llm::oauth_config::google_oauth_config();
        match oauth_server::run_oauth_flow(&config, headless, t).await {
            Ok(_) => {
                println!("  {}", t.oauth_login_success);
                return Ok(String::new());
            }
            Err(e) => {
                tracing::warn!("Browser OAuth failed: {}", e);
                println!("  {}", t.oauth_login_failed);
            }
        }
    }

    // 3. Fallback: API key input
    prompt_api_key(provider, lang, t)
}

/// Resolve Google AI Pro auth: Cratos OAuth → browser login → API key fallback.
async fn resolve_google_pro_auth(
    provider: &providers::Provider,
    lang: Language,
    t: &i18n::Texts,
) -> anyhow::Result<String> {
    use cratos_llm::cli_auth::*;

    // 1. Check existing Cratos Google AI Pro OAuth token
    match check_cratos_google_pro_oauth_status() {
        CratosOAuthStatus::Valid => {
            println!("\n  {} {}", t.oauth_detected, t.oauth_token_valid);

            // Ask if user wants to reuse existing token
            if prompts::confirm(t.oauth_reuse_prompt, true, None)? {
                return Ok(String::new());
            }
        }
        CratosOAuthStatus::Expired => {
            println!("\n  {} {}", t.oauth_detected, t.oauth_token_expired);

            if prompts::confirm(t.oauth_reuse_prompt, true, None)? {
                println!("  {}", t.oauth_refreshing);

                if let Some(tokens) = read_cratos_google_pro_oauth() {
                    if let Some(ref rt) = tokens.refresh_token {
                        let config = cratos_llm::oauth_config::google_pro_oauth_config();
                        match oauth_server::refresh_and_save(&config, rt).await {
                            Ok(_) => {
                                println!("  {}", t.oauth_refresh_success);
                                return Ok(String::new());
                            }
                            Err(e) => {
                                tracing::warn!("Google AI Pro OAuth refresh failed: {}", e);
                            }
                        }
                    }
                }
                println!("  {}", t.oauth_refresh_failed);
            }
        }
        CratosOAuthStatus::NotFound => {}
    }

    // 2. Browser OAuth login
    let headless = is_headless();

    if headless {
        println!("\n  {}", t.oauth_remote_detected);
    }

    println!("\n  {}", t.oauth_browser_login_google);
    println!("  {}", t.oauth_restricted_client_hint);

    if prompts::confirm(t.oauth_browser_login_prompt, true, None)? {
        println!("  {}", t.oauth_starting);

        let config = cratos_llm::oauth_config::google_pro_oauth_config();
        match oauth_server::run_oauth_flow(&config, headless, t).await {
            Ok(_) => {
                println!("  {}", t.oauth_login_success);
                return Ok(String::new());
            }
            Err(e) => {
                tracing::warn!("Browser OAuth failed: {}", e);
                println!("  {}", t.oauth_login_failed);
            }
        }
    }

    // 3. Fallback: API key input
    prompt_api_key(provider, lang, t)
}

/// Resolve OpenAI auth: Cratos OAuth → browser login → API key fallback.
async fn resolve_openai_auth(
    provider: &providers::Provider,
    lang: Language,
    t: &i18n::Texts,
) -> anyhow::Result<String> {
    use cratos_llm::cli_auth::*;

    // 1. Check existing Cratos OAuth token
    match check_cratos_openai_oauth_status() {
        CratosOAuthStatus::Valid => {
            println!("\n  {} {}", t.oauth_detected, t.oauth_token_valid);

            // Ask if user wants to reuse existing token
            if prompts::confirm(t.oauth_reuse_prompt, true, None)? {
                return Ok(String::new());
            }
        }
        CratosOAuthStatus::Expired => {
            println!("\n  {} {}", t.oauth_detected, t.oauth_token_expired);
            println!("  {}", t.oauth_refreshing);

            if let Some(tokens) = read_cratos_openai_oauth() {
                if let Some(ref rt) = tokens.refresh_token {
                    let config = cratos_llm::oauth_config::openai_oauth_config();
                    match oauth_server::refresh_and_save(&config, rt).await {
                        Ok(_) => {
                            println!("  {}", t.oauth_refresh_success);
                            return Ok(String::new());
                        }
                        Err(e) => {
                            tracing::warn!("OpenAI OAuth refresh failed: {}", e);
                        }
                    }
                }
            }
            println!("  {}", t.oauth_refresh_failed);
        }
        CratosOAuthStatus::NotFound => {}
    }

    // 2. Browser OAuth login
    let headless = is_headless();

    if headless {
        println!("\n  {}", t.oauth_remote_detected);
    }

    println!("\n  {}", t.oauth_browser_login_openai);
    if prompts::confirm(t.oauth_browser_login_prompt, true, None)? {
        println!("  {}", t.oauth_starting);

        let config = cratos_llm::oauth_config::openai_oauth_config();
        match oauth_server::run_oauth_flow(&config, headless, t).await {
            Ok(_) => {
                println!("  {}", t.oauth_login_success);
                return Ok(String::new());
            }
            Err(e) => {
                tracing::warn!("Browser OAuth failed: {}", e);
                println!("  {}", t.oauth_login_failed);
            }
        }
    }

    // 3. Fallback: API key input
    prompt_api_key(provider, lang, t)
}
