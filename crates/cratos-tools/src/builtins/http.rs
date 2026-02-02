//! HTTP tools - GET and POST requests

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::time::Instant;
use tracing::debug;

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
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let definition = ToolDefinition::new("http_get", "Make an HTTP GET request")
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

        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'url' parameter".to_string()))?;

        debug!(url = %url, "Making HTTP GET request");

        let mut request = self.client.get(url);

        // Add custom headers
        if let Some(headers) = input.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in headers {
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
                "url": url
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
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let definition = ToolDefinition::new("http_post", "Make an HTTP POST request")
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

        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'url' parameter".to_string()))?;

        let body = input.get("body").and_then(|v| v.as_str()).unwrap_or("");

        let content_type = input
            .get("content_type")
            .and_then(|v| v.as_str())
            .unwrap_or("application/json");

        debug!(url = %url, "Making HTTP POST request");

        let mut request = self
            .client
            .post(url)
            .header("Content-Type", content_type)
            .body(body.to_string());

        // Add custom headers
        if let Some(headers) = input.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in headers {
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
                "url": url
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
}
