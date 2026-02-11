//! Security Audit — automated diagnostic checks for Cratos configuration.
//!
//! Provides a trait-based audit system with 8 configurable checks:
//! 1. AuthEnabledCheck — server.auth.enabled
//! 2. RateLimitCheck — rate_limit settings
//! 3. SandboxCheck — Docker sandbox settings
//! 4. SensitivePathCheck — blocked_paths blacklist
//! 5. CredentialBackendCheck — OS keychain vs plaintext
//! 6. InjectionProtectionCheck — injection detector enabled
//! 7. E2eEncryptionCheck — E2E encryption available
//! 8. ToolPolicyCheck — 6-level policy configured

use serde::Serialize;

/// Severity of an audit finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Pass,
    Info,
    Warning,
    Critical,
}

/// A single audit finding.
#[derive(Debug, Clone, Serialize)]
pub struct AuditFinding {
    pub check_name: String,
    pub severity: Severity,
    pub message: String,
    pub recommendation: Option<String>,
}

/// Full audit report.
#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    pub findings: Vec<AuditFinding>,
    pub summary: AuditSummary,
}

/// Summary counts for the report.
#[derive(Debug, Clone, Serialize)]
pub struct AuditSummary {
    pub total: usize,
    pub pass: usize,
    pub info: usize,
    pub warnings: usize,
    pub critical: usize,
}

impl AuditReport {
    fn from_findings(findings: Vec<AuditFinding>) -> Self {
        let summary = AuditSummary {
            total: findings.len(),
            pass: findings.iter().filter(|f| f.severity == Severity::Pass).count(),
            info: findings.iter().filter(|f| f.severity == Severity::Info).count(),
            warnings: findings.iter().filter(|f| f.severity == Severity::Warning).count(),
            critical: findings
                .iter()
                .filter(|f| f.severity == Severity::Critical)
                .count(),
        };
        Self { findings, summary }
    }

    /// Overall status: "pass" if no warnings/critical, otherwise "fail".
    pub fn status(&self) -> &'static str {
        if self.summary.warnings == 0 && self.summary.critical == 0 {
            "pass"
        } else {
            "fail"
        }
    }
}

/// Configuration snapshot for audit checks.
#[derive(Debug, Clone, Default)]
pub struct AuditInput {
    pub auth_enabled: bool,
    pub rate_limit_enabled: bool,
    pub rate_limit_rpm: u64,
    pub sandbox_available: bool,
    pub sandbox_image: Option<String>,
    pub blocked_paths: Vec<String>,
    pub credential_backend: String,
    pub injection_protection: bool,
    pub e2e_available: bool,
    pub tool_policy_rules: usize,
}

/// Run all audit checks and produce a report.
pub fn run_audit(input: &AuditInput) -> AuditReport {
    let checks: Vec<Box<dyn AuditCheck>> = vec![
        Box::new(AuthEnabledCheck),
        Box::new(RateLimitCheck),
        Box::new(SandboxCheck),
        Box::new(SensitivePathCheck),
        Box::new(CredentialBackendCheck),
        Box::new(InjectionProtectionCheck),
        Box::new(E2eEncryptionCheck),
        Box::new(ToolPolicyCheck),
    ];

    let findings: Vec<AuditFinding> = checks.iter().map(|c| c.check(input)).collect();
    AuditReport::from_findings(findings)
}

/// Trait for individual audit checks.
trait AuditCheck {
    fn check(&self, input: &AuditInput) -> AuditFinding;
}

// ── Check Implementations ──

struct AuthEnabledCheck;

impl AuditCheck for AuthEnabledCheck {
    fn check(&self, input: &AuditInput) -> AuditFinding {
        if input.auth_enabled {
            AuditFinding {
                check_name: "auth_enabled".to_string(),
                severity: Severity::Pass,
                message: "Authentication is enabled".to_string(),
                recommendation: None,
            }
        } else {
            AuditFinding {
                check_name: "auth_enabled".to_string(),
                severity: Severity::Warning,
                message: "Authentication is DISABLED".to_string(),
                recommendation: Some(
                    "Enable [server.auth] enabled = true for production".to_string(),
                ),
            }
        }
    }
}

struct RateLimitCheck;

impl AuditCheck for RateLimitCheck {
    fn check(&self, input: &AuditInput) -> AuditFinding {
        if !input.rate_limit_enabled {
            AuditFinding {
                check_name: "rate_limit".to_string(),
                severity: Severity::Warning,
                message: "Rate limiting is DISABLED".to_string(),
                recommendation: Some(
                    "Enable [server.rate_limit] enabled = true".to_string(),
                ),
            }
        } else if input.rate_limit_rpm > 300 {
            AuditFinding {
                check_name: "rate_limit".to_string(),
                severity: Severity::Info,
                message: format!("Rate limit is high: {} rpm", input.rate_limit_rpm),
                recommendation: Some("Consider lowering to < 120 rpm for production".to_string()),
            }
        } else {
            AuditFinding {
                check_name: "rate_limit".to_string(),
                severity: Severity::Pass,
                message: format!("Rate limiting enabled ({} rpm)", input.rate_limit_rpm),
                recommendation: None,
            }
        }
    }
}

struct SandboxCheck;

impl AuditCheck for SandboxCheck {
    fn check(&self, input: &AuditInput) -> AuditFinding {
        if input.sandbox_available {
            AuditFinding {
                check_name: "sandbox".to_string(),
                severity: Severity::Pass,
                message: format!(
                    "Docker sandbox configured (image: {})",
                    input.sandbox_image.as_deref().unwrap_or("default")
                ),
                recommendation: None,
            }
        } else {
            AuditFinding {
                check_name: "sandbox".to_string(),
                severity: Severity::Info,
                message: "Docker sandbox not configured".to_string(),
                recommendation: Some(
                    "Configure [security.exec] sandbox_image for isolated execution".to_string(),
                ),
            }
        }
    }
}

struct SensitivePathCheck;

impl AuditCheck for SensitivePathCheck {
    fn check(&self, input: &AuditInput) -> AuditFinding {
        let required = ["/etc", "/root", "/dev", "/proc", "/sys"];
        let missing: Vec<&&str> = required
            .iter()
            .filter(|p| !input.blocked_paths.iter().any(|b| b == **p))
            .collect();

        if missing.is_empty() {
            AuditFinding {
                check_name: "sensitive_paths".to_string(),
                severity: Severity::Pass,
                message: format!("{} paths blocked", input.blocked_paths.len()),
                recommendation: None,
            }
        } else {
            AuditFinding {
                check_name: "sensitive_paths".to_string(),
                severity: Severity::Warning,
                message: format!(
                    "Missing blocked paths: {}",
                    missing.iter().map(|p| **p).collect::<Vec<_>>().join(", ")
                ),
                recommendation: Some(
                    "Add missing paths to [security.exec] blocked_paths".to_string(),
                ),
            }
        }
    }
}

struct CredentialBackendCheck;

impl AuditCheck for CredentialBackendCheck {
    fn check(&self, input: &AuditInput) -> AuditFinding {
        match input.credential_backend.as_str() {
            "keychain" => AuditFinding {
                check_name: "credential_backend".to_string(),
                severity: Severity::Pass,
                message: "Using OS keychain for credential storage".to_string(),
                recommendation: None,
            },
            "encrypted_file" => AuditFinding {
                check_name: "credential_backend".to_string(),
                severity: Severity::Info,
                message: "Using encrypted file for credential storage".to_string(),
                recommendation: Some("OS keychain is recommended for production".to_string()),
            },
            _ => AuditFinding {
                check_name: "credential_backend".to_string(),
                severity: Severity::Warning,
                message: format!(
                    "Credential backend '{}' may store secrets in plaintext",
                    input.credential_backend
                ),
                recommendation: Some(
                    "Use 'keychain' or 'encrypted_file' for [server.auth] key_storage".to_string(),
                ),
            },
        }
    }
}

struct InjectionProtectionCheck;

impl AuditCheck for InjectionProtectionCheck {
    fn check(&self, input: &AuditInput) -> AuditFinding {
        if input.injection_protection {
            AuditFinding {
                check_name: "injection_protection".to_string(),
                severity: Severity::Pass,
                message: "Prompt injection protection is enabled".to_string(),
                recommendation: None,
            }
        } else {
            AuditFinding {
                check_name: "injection_protection".to_string(),
                severity: Severity::Warning,
                message: "Prompt injection protection is DISABLED".to_string(),
                recommendation: Some(
                    "Enable [security] enable_injection_protection = true".to_string(),
                ),
            }
        }
    }
}

struct E2eEncryptionCheck;

impl AuditCheck for E2eEncryptionCheck {
    fn check(&self, input: &AuditInput) -> AuditFinding {
        if input.e2e_available {
            AuditFinding {
                check_name: "e2e_encryption".to_string(),
                severity: Severity::Pass,
                message: "E2E encryption (X25519+AES-256-GCM) is available".to_string(),
                recommendation: None,
            }
        } else {
            AuditFinding {
                check_name: "e2e_encryption".to_string(),
                severity: Severity::Info,
                message: "E2E encryption not configured".to_string(),
                recommendation: Some(
                    "E2E is available via /api/v1/sessions/init-e2e endpoint".to_string(),
                ),
            }
        }
    }
}

struct ToolPolicyCheck;

impl AuditCheck for ToolPolicyCheck {
    fn check(&self, input: &AuditInput) -> AuditFinding {
        if input.tool_policy_rules > 0 {
            AuditFinding {
                check_name: "tool_policy".to_string(),
                severity: Severity::Pass,
                message: format!(
                    "6-level tool security policy active ({} rules)",
                    input.tool_policy_rules
                ),
                recommendation: None,
            }
        } else {
            AuditFinding {
                check_name: "tool_policy".to_string(),
                severity: Severity::Info,
                message: "No custom tool security policy configured".to_string(),
                recommendation: Some(
                    "Configure ToolSecurityPolicy for fine-grained tool access control".to_string(),
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_pass() {
        let input = AuditInput {
            auth_enabled: true,
            rate_limit_enabled: true,
            rate_limit_rpm: 60,
            sandbox_available: true,
            sandbox_image: Some("ubuntu:latest".to_string()),
            blocked_paths: vec![
                "/etc".to_string(),
                "/root".to_string(),
                "/dev".to_string(),
                "/proc".to_string(),
                "/sys".to_string(),
            ],
            credential_backend: "keychain".to_string(),
            injection_protection: true,
            e2e_available: true,
            tool_policy_rules: 4,
        };
        let report = run_audit(&input);
        assert_eq!(report.status(), "pass");
        assert_eq!(report.summary.total, 8);
        assert_eq!(report.summary.pass, 8);
    }

    #[test]
    fn test_all_fail() {
        let input = AuditInput::default();
        let report = run_audit(&input);
        assert_eq!(report.status(), "fail");
        assert!(report.summary.warnings > 0);
    }

    #[test]
    fn test_auth_disabled_warning() {
        let input = AuditInput {
            auth_enabled: false,
            ..Default::default()
        };
        let report = run_audit(&input);
        let auth_finding = report
            .findings
            .iter()
            .find(|f| f.check_name == "auth_enabled")
            .unwrap();
        assert_eq!(auth_finding.severity, Severity::Warning);
    }

    #[test]
    fn test_high_rate_limit_info() {
        let input = AuditInput {
            rate_limit_enabled: true,
            rate_limit_rpm: 500,
            ..Default::default()
        };
        let report = run_audit(&input);
        let rl_finding = report
            .findings
            .iter()
            .find(|f| f.check_name == "rate_limit")
            .unwrap();
        assert_eq!(rl_finding.severity, Severity::Info);
    }

    #[test]
    fn test_missing_blocked_paths() {
        let input = AuditInput {
            blocked_paths: vec!["/etc".to_string()],
            ..Default::default()
        };
        let report = run_audit(&input);
        let path_finding = report
            .findings
            .iter()
            .find(|f| f.check_name == "sensitive_paths")
            .unwrap();
        assert_eq!(path_finding.severity, Severity::Warning);
    }

    #[test]
    fn test_report_serialization() {
        let input = AuditInput::default();
        let report = run_audit(&input);
        let json = serde_json::to_string_pretty(&report).unwrap();
        assert!(json.contains("\"check_name\""));
        assert!(json.contains("\"summary\""));
    }
}
