use super::*;
use crate::registry::Tool;
use std::path::Path;
use super::security;

#[test]
fn test_file_read_definition() {
    let tool = FileReadTool::new();
    let def = tool.definition();

    assert_eq!(def.name, "file_read");
    assert_eq!(def.risk_level, RiskLevel::Low);
    assert_eq!(def.category, ToolCategory::File);
}

#[tokio::test]
async fn test_file_read_missing_path() {
    let tool = FileReadTool::new();
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_err());
}

#[test]
fn test_path_traversal_blocked() {
    assert!(security::validate_path("../../../etc/passwd").is_err());
    assert!(security::validate_path("/etc/passwd").is_err());
    assert!(security::validate_path("/root/.ssh/id_rsa").is_err());
    assert!(security::validate_path("/var/log/syslog").is_err());

    #[cfg(unix)]
    assert!(security::validate_path("/tmp").is_ok());
}

#[test]
fn test_symlink_attack_prevention() {
    assert!(security::validate_path("/etc").is_err());
    assert!(security::validate_path("/root").is_err());
    assert!(security::validate_path("/var/log").is_err());
    assert!(security::validate_path("/boot").is_err());
    assert!(security::validate_path("/proc").is_err());
    assert!(security::validate_path("/sys").is_err());

    assert!(security::validate_path("/tmp/../etc/passwd").is_err());
}

#[test]
fn test_sensitive_file_detection() {
    assert!(security::is_sensitive_file(Path::new(".env")));
    assert!(security::is_sensitive_file(Path::new(".env.production")));
    assert!(security::is_sensitive_file(Path::new("credentials.json")));
    assert!(security::is_sensitive_file(Path::new("id_rsa")));
    assert!(security::is_sensitive_file(Path::new("api_key.txt")));

    assert!(!security::is_sensitive_file(Path::new("main.rs")));
    assert!(!security::is_sensitive_file(Path::new("config.toml")));
    assert!(!security::is_sensitive_file(Path::new("README.md")));
}

#[test]
fn test_sensitive_content_detection() {
    assert!(security::content_appears_sensitive("API_KEY=sk-1234567890"));
    assert!(security::content_appears_sensitive("password=secret123"));
    assert!(security::content_appears_sensitive("Bearer eyJhbGciOiJIUzI1NiJ9"));
    assert!(security::content_appears_sensitive("-----BEGIN RSA PRIVATE KEY-----"));
    assert!(security::content_appears_sensitive(
        "aws_secret_access_key=AKIAIOSFODNN7EXAMPLE"
    ));

    assert!(!security::content_appears_sensitive("Hello, world!"));
    assert!(!security::content_appears_sensitive(
        "fn main() { println!(\"Hello\"); }"
    ));
    assert!(!security::content_appears_sensitive("# Configuration\nport = 8080"));
}

#[tokio::test]
async fn test_file_read_blocks_sensitive() {
    let tool = FileReadTool::new();

    let result = tool
        .execute(serde_json::json!({
            "path": "/home/user/.env"
        }))
        .await;
    assert!(result.is_err());

    let result = tool
        .execute(serde_json::json!({
            "path": "/etc/passwd"
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_write_blocks_secrets() {
    let tool = FileWriteTool::new();

    let result = tool
        .execute(serde_json::json!({
            "path": "/tmp/test.txt",
            "content": "API_KEY=sk-secret123"
        }))
        .await;
    assert!(result.is_err());
}
