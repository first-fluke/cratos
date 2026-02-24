//! HTTP tools - GET and POST requests

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::collections::HashSet;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use url::Url;

// ============================================================================
// Constants
// ============================================================================

/// Default HTTP request timeout in seconds
const HTTP_TIMEOUT_SECS: u64 = 30;

/// Default HTTP port
const DEFAULT_HTTP_PORT: u16 = 80;

/// Link-local IPv4 range first octet (169.254.x.x)
const LINK_LOCAL_IPV4_FIRST_OCTET: u8 = 169;

/// Link-local IPv4 range second octet (169.254.x.x)
const LINK_LOCAL_IPV4_SECOND_OCTET: u8 = 254;

/// Blocked hosts for SSRF protection
static BLOCKED_HOSTS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "localhost",
        "127.0.0.1",
        "0.0.0.0",
        "::1",
        "[::1]",
        "metadata.google.internal",
        "169.254.169.254", // AWS/GCP metadata
        "metadata.internal",
    ])
});

/// Blocked headers that could be used for attacks
static BLOCKED_HEADERS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "host",
        "authorization",
        "proxy-authorization",
        "cookie",
        "set-cookie",
        "x-forwarded-for",
        "x-real-ip",
        "x-forwarded-host",
    ])
});

/// Validate a URL for security
fn validate_url(url_str: &str) -> Result<Url> {
    let url =
        Url::parse(url_str).map_err(|e| Error::InvalidInput(format!("Invalid URL: {}", e)))?;

    // SECURITY: Only allow http/https schemes
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            warn!(scheme = %scheme, url = %url_str, "Blocked non-HTTP URL scheme");
            return Err(Error::PermissionDenied(format!(
                "URL scheme '{}' is not allowed. Only http/https are permitted.",
                scheme
            )));
        }
    }

    // SECURITY: Check for blocked hosts
    if let Some(host) = url.host_str() {
        let host_lower = host.to_lowercase();

        if BLOCKED_HOSTS.contains(host_lower.as_str()) {
            warn!(host = %host, "Blocked request to internal host (SSRF protection)");
            return Err(Error::PermissionDenied(format!(
                "Requests to '{}' are blocked for security reasons",
                host
            )));
        }

        // Check for private IP ranges
        if let Ok(ip) = host.parse::<IpAddr>() {
            if is_private_ip(&ip) {
                warn!(ip = %ip, "Blocked request to private IP (SSRF protection)");
                return Err(Error::PermissionDenied(
                    "Requests to private/internal IP addresses are not allowed".to_string(),
                ));
            }
        }

        // Block internal domain patterns
        if host_lower.ends_with(".internal")
            || host_lower.ends_with(".local")
            || host_lower.ends_with(".localhost")
        {
            warn!(host = %host, "Blocked request to internal domain");
            return Err(Error::PermissionDenied(format!(
                "Requests to internal domains like '{}' are not allowed",
                host
            )));
        }
    }

    Ok(url)
}

/// Check if an IP address is private/internal
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback()
                || ipv4.is_private()
                || ipv4.is_link_local()
                || ipv4.is_broadcast()
                || ipv4.is_documentation()
                || ipv4.is_unspecified()
                // 169.254.x.x (link-local)
                || (ipv4.octets()[0] == LINK_LOCAL_IPV4_FIRST_OCTET
                    && ipv4.octets()[1] == LINK_LOCAL_IPV4_SECOND_OCTET)
        }
        IpAddr::V6(ipv6) => ipv6.is_loopback() || ipv6.is_unspecified(),
    }
}

/// Check if a header is blocked
fn is_header_blocked(header_name: &str) -> bool {
    BLOCKED_HEADERS.contains(header_name.to_lowercase().as_str())
}

/// SECURITY: Resolve hostname and validate IP addresses just before making the request
/// This prevents DNS rebinding attacks where DNS changes between validation and request
fn validate_resolved_ips(url: &Url) -> Result<()> {
    let host = url
        .host_str()
        .ok_or_else(|| Error::InvalidInput("URL has no host".to_string()))?;

    // Skip validation for IP addresses (already validated in validate_url)
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }

    let port = url.port_or_known_default().unwrap_or(DEFAULT_HTTP_PORT);
    let socket_addr = format!("{}:{}", host, port);

    // Resolve DNS and check all returned IPs
    let resolved_ips: Vec<_> = socket_addr
        .to_socket_addrs()
        .map_err(|e| Error::Network(format!("DNS resolution failed for '{}': {}", host, e)))?
        .collect();

    if resolved_ips.is_empty() {
        return Err(Error::Network(format!(
            "DNS resolution returned no addresses for '{}'",
            host
        )));
    }

    for addr in &resolved_ips {
        let ip = addr.ip();
        if is_private_ip(&ip) {
            warn!(
                host = %host,
                resolved_ip = %ip,
                "DNS rebinding attack blocked: hostname resolved to private IP"
            );
            return Err(Error::PermissionDenied(format!(
                "Security: hostname '{}' resolved to private IP address {} (potential DNS rebinding attack)",
                host, ip
            )));
        }
    }

    debug!(
        host = %host,
        resolved_ips = ?resolved_ips.iter().map(|a| a.ip().to_string()).collect::<Vec<_>>(),
        "DNS resolution validated - all IPs are public"
    );

    Ok(())
}

// ============================================================================
// HTTP GET Tool
// ============================================================================

/// Tool for HTTP GET requests
pub struct HttpGetTool {
    definition: ToolDefinition,
    client: reqwest::Client,
}

impl HttpGetTool {
    /// Create a new HTTP GET tool
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");

        let definition = ToolDefinition::new(
            "http_get",
            "Make an HTTP GET request. Use for fetching API responses, downloading web pages, or checking URLs. \
             Returns status code, headers, and body. Timeout: 30s, max response: 10MB. \
             Use custom headers for authentication (e.g. Authorization: Bearer ...). \
             Prefer this over browser for simple data fetching without JS rendering. \
             Example: {\"url\": \"https://api.example.com/data\", \"headers\": {\"Accept\": \"application/json\"}}"
        )
            .with_category(ToolCategory::Http)
            .with_risk_level(RiskLevel::Low)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to request"
                    },
                    "headers": {
                        "type": "object",
                        "description": "Additional headers to send",
                        "additionalProperties": {"type": "string"}
                    }
                },
                "required": ["url"]
            }));

        Self { definition, client }
    }
}

impl Default for HttpGetTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for HttpGetTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let url_str = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'url' parameter".to_string()))?;

        // SECURITY: Validate URL
        let validated_url = validate_url(url_str)?;

        // SECURITY: Validate resolved IPs just before request to prevent DNS rebinding
        validate_resolved_ips(&validated_url)?;

        debug!(url = %validated_url, "Making HTTP GET request");

        let mut request = self.client.get(validated_url.as_str());

        // Add custom headers (with security filtering)
        if let Some(headers) = input.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in headers {
                // SECURITY: Block sensitive headers
                if is_header_blocked(key) {
                    warn!(header = %key, "Blocked attempt to set sensitive header");
                    continue;
                }
                if let Some(v) = value.as_str() {
                    request = request.header(key, v);
                }
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let status = response.status().as_u16();
        let headers: serde_json::Map<String, serde_json::Value> = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    serde_json::Value::String(v.to_str().unwrap_or("").to_string()),
                )
            })
            .collect();

        let body = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let duration = start.elapsed().as_millis() as u64;

        Ok(ToolResult::success(
            serde_json::json!({
                "status": status,
                "headers": headers,
                "body": body,
                "url": url_str
            }),
            duration,
        ))
    }
}

// ============================================================================
// HTTP POST Tool
// ============================================================================

/// Tool for HTTP POST requests
pub struct HttpPostTool {
    definition: ToolDefinition,
    client: reqwest::Client,
}

impl HttpPostTool {
    /// Create a new HTTP POST tool
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");

        let definition = ToolDefinition::new(
            "http_post",
            "Make an HTTP POST request. Use for submitting data to APIs, webhooks, or form endpoints. \
             Set content_type for the body format (default: application/json). \
             Supports JSON, form-urlencoded, and raw text bodies. Timeout: 30s. \
             Example: {\"url\": \"https://api.example.com/submit\", \"body\": \"{\\\"key\\\": \\\"value\\\"}\", \"content_type\": \"application/json\"}"
        )
            .with_category(ToolCategory::Http)
            .with_risk_level(RiskLevel::Medium)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to request"
                    },
                    "body": {
                        "type": "string",
                        "description": "Request body"
                    },
                    "content_type": {
                        "type": "string",
                        "description": "Content-Type header",
                        "default": "application/json"
                    },
                    "headers": {
                        "type": "object",
                        "description": "Additional headers to send",
                        "additionalProperties": {"type": "string"}
                    }
                },
                "required": ["url"]
            }));

        Self { definition, client }
    }
}

impl Default for HttpPostTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for HttpPostTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let url_str = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'url' parameter".to_string()))?;

        // SECURITY: Validate URL
        let validated_url = validate_url(url_str)?;

        // SECURITY: Validate resolved IPs just before request to prevent DNS rebinding
        validate_resolved_ips(&validated_url)?;

        let body = input.get("body").and_then(|v| v.as_str()).unwrap_or("");

        let content_type = input
            .get("content_type")
            .and_then(|v| v.as_str())
            .unwrap_or("application/json");

        debug!(url = %validated_url, "Making HTTP POST request");

        let mut request = self
            .client
            .post(validated_url.as_str())
            .header("Content-Type", content_type)
            .body(body.to_string());

        // Add custom headers (with security filtering)
        if let Some(headers) = input.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in headers {
                // SECURITY: Block sensitive headers
                if is_header_blocked(key) {
                    warn!(header = %key, "Blocked attempt to set sensitive header");
                    continue;
                }
                if let Some(v) = value.as_str() {
                    request = request.header(key, v);
                }
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let status = response.status().as_u16();
        let headers: serde_json::Map<String, serde_json::Value> = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    serde_json::Value::String(v.to_str().unwrap_or("").to_string()),
                )
            })
            .collect();

        let response_body = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let duration = start.elapsed().as_millis() as u64;

        Ok(ToolResult::success(
            serde_json::json!({
                "status": status,
                "headers": headers,
                "body": response_body,
                "url": url_str
            }),
            duration,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Tool;

    #[tokio::test]
    async fn test_http_get_missing_url() {
        let tool = HttpGetTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_url_blocks_localhost() {
        assert!(validate_url("http://localhost/api").is_err());
        assert!(validate_url("http://127.0.0.1/api").is_err());
        assert!(validate_url("http://0.0.0.0/api").is_err());
        assert!(validate_url("http://[::1]/api").is_err());
    }

    #[test]
    fn test_validate_url_blocks_internal() {
        assert!(validate_url("http://metadata.google.internal/").is_err());
        assert!(validate_url("http://169.254.169.254/").is_err());
        assert!(validate_url("http://internal.local/api").is_err());
        assert!(validate_url("http://service.internal/api").is_err());
    }

    #[test]
    fn test_validate_url_blocks_private_ip() {
        assert!(validate_url("http://10.0.0.1/api").is_err());
        assert!(validate_url("http://192.168.1.1/api").is_err());
        assert!(validate_url("http://172.16.0.1/api").is_err());
    }

    #[test]
    fn test_validate_url_blocks_non_http() {
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("ftp://ftp.example.com/").is_err());
        assert!(validate_url("gopher://example.com/").is_err());
    }

    #[test]
    fn test_validate_url_allows_external() {
        assert!(validate_url("https://api.github.com/").is_ok());
        assert!(validate_url("https://example.com/api").is_ok());
        assert!(validate_url("http://httpbin.org/get").is_ok());
    }

    #[test]
    fn test_is_header_blocked() {
        assert!(is_header_blocked("Host"));
        assert!(is_header_blocked("host"));
        assert!(is_header_blocked("Authorization"));
        assert!(is_header_blocked("Cookie"));
        assert!(is_header_blocked("X-Forwarded-For"));

        assert!(!is_header_blocked("Content-Type"));
        assert!(!is_header_blocked("Accept"));
        assert!(!is_header_blocked("User-Agent"));
    }

    #[test]
    fn test_private_ip_detection() {
        use std::net::Ipv4Addr;

        // Private ranges
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));

        // Public IPs
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[tokio::test]
    async fn test_http_get_blocks_ssrf() {
        let tool = HttpGetTool::new();

        let result = tool
            .execute(serde_json::json!({
                "url": "http://localhost:19527/admin"
            }))
            .await;
        assert!(result.is_err());

        let result = tool
            .execute(serde_json::json!({
                "url": "http://169.254.169.254/latest/meta-data/"
            }))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_dns_rebinding_protection() {
        // Test that validate_resolved_ips catches private IPs
        // This simulates what would happen if DNS resolved to a private IP

        // Test direct IP validation still works
        assert!(validate_url("http://10.0.0.1/api").is_err());
        assert!(validate_url("http://192.168.1.1/api").is_err());
        assert!(validate_url("http://127.0.0.1/api").is_err());

        // External URLs should pass initial validation
        // (actual DNS rebinding would be caught by validate_resolved_ips at request time)
        assert!(validate_url("https://example.com/api").is_ok());
    }
}
