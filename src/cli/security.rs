//! Security CLI subcommand — `cratos security audit`

use anyhow::Result;
use cratos_core::security::audit::{run_audit, AuditInput, Severity};

/// Run the security audit CLI command.
pub async fn run_audit_cli(json: bool) -> Result<()> {
    let config = crate::server::load_config()?;

    let input = AuditInput {
        auth_enabled: config.server.auth.enabled,
        rate_limit_enabled: config.server.rate_limit.enabled,
        rate_limit_rpm: config.server.rate_limit.requests_per_minute as u64,
        sandbox_available: config.security.exec.mode.as_str() == "strict"
            || std::process::Command::new("docker")
                .arg("--version")
                .output()
                .is_ok(),
        sandbox_image: None,
        blocked_paths: config.security.exec.blocked_paths.clone(),
        credential_backend: config.server.auth.key_storage.clone(),
        injection_protection: config.security.enable_injection_protection.unwrap_or(false),
        e2e_available: true,  // Always available since Phase 1.1
        tool_policy_rules: 0, // Would need runtime policy to check
    };

    let report = run_audit(&input);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Cratos Security Audit");
        println!("=====================\n");

        for finding in &report.findings {
            let icon = match finding.severity {
                Severity::Pass => "[PASS]",
                Severity::Info => "[INFO]",
                Severity::Warning => "[WARN]",
                Severity::Critical => "[CRIT]",
            };
            println!("  {} {}: {}", icon, finding.check_name, finding.message);
            if let Some(ref rec) = finding.recommendation {
                println!("         -> {}", rec);
            }
        }

        println!(
            "\nSummary: {} checks, {} pass, {} warnings, {} critical",
            report.summary.total,
            report.summary.pass,
            report.summary.warnings,
            report.summary.critical,
        );
        println!("Status: {}", report.status().to_uppercase());
    }

    Ok(())
}
