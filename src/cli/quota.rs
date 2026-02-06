//! CLI command: `cratos quota`
//!
//! Displays per-provider rate limit status (requests, tokens, reset time)
//! and today's cumulative cost from the CostTracker.

use cratos_llm::{format_compact_number, format_duration, global_quota_tracker, global_tracker};

/// Run the quota subcommand.
pub async fn run(json: bool, watch: bool) -> anyhow::Result<()> {
    if watch {
        run_watch().await
    } else if json {
        run_json().await
    } else {
        run_table().await
    }
}

/// Pretty-printed table output.
async fn run_table() -> anyhow::Result<()> {
    let tracker = global_quota_tracker();
    let cost_tracker = global_tracker();
    let mut states = tracker.get_all_states().await;
    states.sort_by(|a, b| a.provider.cmp(&b.provider));

    println!();
    println!("  Provider Quota");
    println!("  {}", "-".repeat(72));
    println!(
        "  {:<24} {:<18} {:<22} Reset In",
        "Provider", "Requests", "Tokens"
    );
    println!("  {}", "-".repeat(72));

    if states.is_empty() {
        println!("  (no API calls recorded yet)");
    } else {
        for state in &states {
            let auth_suffix = cratos_llm::cli_auth::get_auth_source(&state.provider)
                .filter(|s| *s != cratos_llm::cli_auth::AuthSource::ApiKey)
                .map(|s| format!(" ({})", s))
                .unwrap_or_default();
            let display_name = format!("{}{}", state.provider, auth_suffix);

            let req_str = format_ratio(state.requests_remaining, state.requests_limit);
            let tok_str = format_ratio_compact(state.tokens_remaining, state.tokens_limit);
            let reset_str = format_reset(&state.reset_at);

            let warn = if state.is_near_limit(20.0) { " !!" } else { "" };

            println!(
                "  {:<24} {:<18} {:<22} {}{}",
                display_name, req_str, tok_str, reset_str, warn
            );
        }
    }

    // Cost summary from CostTracker
    let report = cost_tracker.generate_report(None).await;
    let total_tokens = report.stats.total_input_tokens + report.stats.total_output_tokens;
    println!("  {}", "-".repeat(72));
    println!(
        "  Today's Cost: ${:.4}  |  Total Tokens: {}",
        report.stats.total_cost,
        format_compact_number(total_tokens)
    );
    println!();

    Ok(())
}

/// JSON output for scripting.
async fn run_json() -> anyhow::Result<()> {
    let tracker = global_quota_tracker();
    let cost_tracker = global_tracker();
    let states = tracker.get_all_states().await;
    let report = cost_tracker.generate_report(None).await;

    let providers: Vec<serde_json::Value> = states
        .iter()
        .map(|s| {
            let reset_in = s
                .reset_at
                .map(|r| (r - chrono::Utc::now()).num_seconds().max(0))
                .unwrap_or(0);

            serde_json::json!({
                "name": s.provider,
                "auth_source": cratos_llm::cli_auth::get_auth_source(&s.provider)
                    .map(|src| src.to_string()),
                "requests": {
                    "remaining": s.requests_remaining,
                    "limit": s.requests_limit,
                    "usage_pct": s.requests_usage_pct(),
                },
                "tokens": {
                    "remaining": s.tokens_remaining,
                    "limit": s.tokens_limit,
                    "usage_pct": s.tokens_usage_pct(),
                },
                "reset_at": s.reset_at.map(|r| r.to_rfc3339()),
                "reset_in_seconds": reset_in,
                "updated_at": s.updated_at.to_rfc3339(),
                "warning": s.is_near_limit(20.0),
            })
        })
        .collect();

    let output = serde_json::json!({
        "providers": providers,
        "today": {
            "total_cost_usd": report.stats.total_cost,
            "total_tokens": report.stats.total_input_tokens + report.stats.total_output_tokens,
        }
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// Watch mode: refresh display every second.
async fn run_watch() -> anyhow::Result<()> {
    loop {
        // Clear screen
        print!("\x1b[2J\x1b[H");
        run_table().await?;
        println!("  (refreshing every 2s, Ctrl+C to exit)");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
}

// ── helpers ──────────────────────────────────────────────────────────────

fn format_ratio(remaining: Option<u64>, limit: Option<u64>) -> String {
    match (remaining, limit) {
        (Some(rem), Some(lim)) => {
            let pct = if lim > 0 {
                ((1.0 - rem as f64 / lim as f64) * 100.0) as u64
            } else {
                0
            };
            format!(
                "{} / {} ({}%)",
                format_number_with_commas(rem),
                format_number_with_commas(lim),
                pct
            )
        }
        _ => "- / -".to_string(),
    }
}

fn format_ratio_compact(remaining: Option<u64>, limit: Option<u64>) -> String {
    match (remaining, limit) {
        (Some(rem), Some(lim)) => {
            let pct = if lim > 0 {
                ((1.0 - rem as f64 / lim as f64) * 100.0) as u64
            } else {
                0
            };
            format!(
                "{} / {} ({}%)",
                format_compact_number(rem),
                format_compact_number(lim),
                pct
            )
        }
        _ => "- / -".to_string(),
    }
}

fn format_reset(reset_at: &Option<chrono::DateTime<chrono::Utc>>) -> String {
    match reset_at {
        Some(at) => {
            let dur = *at - chrono::Utc::now();
            if dur.num_seconds() <= 0 {
                "now".to_string()
            } else {
                format_duration(&dur)
            }
        }
        None => "-".to_string(),
    }
}

fn format_number_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
