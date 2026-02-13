//! Tests for tool doctor module

use super::*;

#[test]
fn test_diagnose_permission_error() {
    let doctor = ToolDoctor::new();
    let diagnosis = doctor.diagnose("file_read", "Error: Permission denied: /etc/shadow");

    assert_eq!(diagnosis.category, FailureCategory::Permission);
    assert!(diagnosis.confidence > 0.8);
    assert!(!diagnosis.probable_causes.is_empty());
    assert!(!diagnosis.checklist.is_empty());
}

#[test]
fn test_diagnose_authentication_error() {
    let doctor = ToolDoctor::new();
    let diagnosis = doctor.diagnose("http_get", "401 Unauthorized: Invalid API key");

    assert_eq!(diagnosis.category, FailureCategory::Authentication);
    assert!(diagnosis.confidence > 0.8);
}

#[test]
fn test_diagnose_network_error() {
    let doctor = ToolDoctor::new();
    let diagnosis = doctor.diagnose("http_get", "Connection refused: server is not responding");

    assert_eq!(diagnosis.category, FailureCategory::Network);
    assert!(diagnosis.confidence > 0.8);
}

#[test]
fn test_diagnose_rate_limit_error() {
    let doctor = ToolDoctor::new();
    let diagnosis = doctor.diagnose(
        "github_create_pr",
        "429 Too Many Requests: rate limit exceeded",
    );

    assert_eq!(diagnosis.category, FailureCategory::RateLimit);
    assert!(diagnosis.confidence > 0.9);
}

#[test]
fn test_diagnose_not_found_error() {
    let doctor = ToolDoctor::new();
    let diagnosis = doctor.diagnose(
        "file_read",
        "Error: No such file or directory: /path/to/missing",
    );

    assert_eq!(diagnosis.category, FailureCategory::NotFound);
    assert!(diagnosis.confidence > 0.8);
}

#[test]
fn test_diagnose_timeout_error() {
    let doctor = ToolDoctor::new();
    let diagnosis =
        doctor.diagnose("http_get", "Request timeout: operation timed out after 30s");

    assert_eq!(diagnosis.category, FailureCategory::Timeout);
    assert!(diagnosis.confidence > 0.8);
}

#[test]
fn test_diagnose_unknown_error() {
    let doctor = ToolDoctor::new();
    let diagnosis = doctor.diagnose("some_tool", "Something went wrong");

    assert_eq!(diagnosis.category, FailureCategory::Unknown);
}

#[test]
fn test_tool_specific_diagnosis() {
    let doctor = ToolDoctor::new();

    // Git clone with directory already exists
    let diagnosis = doctor.diagnose("git_clone", "Error: directory already exists");
    assert_eq!(diagnosis.category, FailureCategory::InvalidInput);
    assert!(diagnosis.confidence > 0.9);
}

#[test]
fn test_checklist_has_items() {
    let doctor = ToolDoctor::new();
    let diagnosis = doctor.diagnose("file_read", "Permission denied");

    assert!(diagnosis.checklist.len() >= 2);
    assert_eq!(diagnosis.checklist[0].step, 1);
}

#[test]
fn test_alternatives_for_http() {
    let doctor = ToolDoctor::new();
    let diagnosis = doctor.diagnose("http_get", "Connection refused");

    assert!(!diagnosis.alternatives.is_empty());
    assert!(diagnosis
        .alternatives
        .iter()
        .any(|a| a.tool_name.as_deref() == Some("exec")));
}

#[test]
fn test_format_diagnosis() {
    let doctor = ToolDoctor::new();
    let diagnosis = doctor.diagnose("file_read", "Permission denied: /etc/shadow");

    let formatted = doctor.format_diagnosis(&diagnosis);

    assert!(formatted.contains("Tool Doctor Diagnosis"));
    assert!(formatted.contains("Permission Denied"));
    assert!(formatted.contains("Probable Causes"));
    assert!(formatted.contains("Resolution Checklist"));
}

#[test]
fn test_failure_category_display() {
    assert_eq!(
        FailureCategory::Permission.display_name(),
        "Permission Denied"
    );
    assert_eq!(
        FailureCategory::Authentication.display_name(),
        "Authentication Failed"
    );
    assert_eq!(FailureCategory::Network.display_name(), "Network Error");
    assert_eq!(
        FailureCategory::RateLimit.display_name(),
        "Rate Limit Exceeded"
    );
}

#[test]
fn test_failure_category_common_causes() {
    let causes = FailureCategory::Permission.common_causes();
    assert!(!causes.is_empty());
    assert!(causes.iter().any(|c| c.contains("permission")));
}
