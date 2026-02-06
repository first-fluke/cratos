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

    let skip_telegram =
        prompts::confirm(t.telegram_skip, true, Some(t.telegram_skip_note))?;

    let telegram_token = if skip_telegram {
        String::new()
    } else {
        prompts::password(t.telegram_prompt)?
    };

    // ── 6. Slack ──
    print_header(t.slack_title);

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
        println!("  {}", t.apikey_ollama_run_en);
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
            prompt_api_key(provider, lang, t)?
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

    println!(
        "\n  Selected: {} ({})\n",
        persona.name, persona.domain
    );

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

    if !api_key.is_empty() || provider.name == "ollama" {
        print!("  {} ", t.test_llm);
        if testing::test_llm(provider, &api_key).await {
            println!("{}", t.test_success);
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
        if telegram_token.is_empty() { t.disabled } else { t.enabled }
    );
    println!(
        "    Slack: {}",
        if slack_token.is_empty() { t.disabled } else { t.enabled }
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

    let instructions = t
        .apikey_instructions
        .replace("{url}", provider.signup_url);
    println!("{}", instructions);
    println!();

    prompts::password_required(t.apikey_prompt)
}
