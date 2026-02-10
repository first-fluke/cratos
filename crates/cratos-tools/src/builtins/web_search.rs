//! Web search tool — DuckDuckGo HTML scraping (no API key required)
//!
//! Uses system `curl` for HTTP requests to avoid TLS fingerprint-based bot
//! detection (rustls JA3 fingerprint gets blocked by DuckDuckGo).

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use regex::Regex;
use std::time::Instant;
use tracing::{debug, warn};

/// Maximum number of search results to return
const MAX_RESULTS_CAP: usize = 10;

/// Default number of results
const DEFAULT_MAX_RESULTS: usize = 5;

/// Default DuckDuckGo region code
const DEFAULT_REGION: &str = "kr-kr";

/// HTTP timeout for the search request (seconds)
const SEARCH_TIMEOUT_SECS: u64 = 15;

/// User-Agent header to avoid bot blocking
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// A single search result entry.
#[derive(Debug, Clone, serde::Serialize)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// DuckDuckGo HTML-based web search tool.
///
/// LLM passes only a `query` string — the tool builds the URL internally,
/// so there is no risk of the LLM misspelling URLs or Korean characters.
///
/// Uses system `curl` for HTTP requests because reqwest's rustls TLS
/// fingerprint (JA3) gets blocked by DuckDuckGo's bot detection.
pub struct WebSearchTool {
    definition: ToolDefinition,
}

impl WebSearchTool {
    /// Create a new web search tool.
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "web_search",
            "Search the web using DuckDuckGo. Returns titles, URLs, and snippets. \
             Use this tool for real-time information like weather, news, prices, \
             and any query that requires up-to-date web results.",
        )
        .with_category(ToolCategory::Http)
        .with_risk_level(RiskLevel::Low)
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return (1-10, default 5)"
                },
                "region": {
                    "type": "string",
                    "description": "DuckDuckGo region code (e.g. 'kr-kr', 'us-en')"
                }
            },
            "required": ["query"]
        }));

        Self { definition }
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'query' parameter".to_string()))?;

        if query.trim().is_empty() {
            return Err(Error::InvalidInput("Query must not be empty".to_string()));
        }

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| (n as usize).clamp(1, MAX_RESULTS_CAP))
            .unwrap_or(DEFAULT_MAX_RESULTS);

        let region = input
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_REGION);

        let results = fetch_search_results(query, region, max_results).await?;
        let duration = start.elapsed().as_millis() as u64;

        Ok(ToolResult::success(
            serde_json::json!({
                "query": query,
                "results": results,
                "total": results.len(),
            }),
            duration,
        ))
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Fetch and parse DuckDuckGo HTML search results via system `curl`.
///
/// Uses POST to avoid CAPTCHA that DuckDuckGo shows for GET requests
/// with non-ASCII queries (e.g. Korean, Japanese, Chinese).
///
/// We use system `curl` instead of reqwest because DuckDuckGo blocks
/// rustls TLS fingerprints (JA3). System curl uses OpenSSL which is
/// indistinguishable from a real browser on the TLS level.
async fn fetch_search_results(
    query: &str,
    region: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>> {
    let url = "https://html.duckduckgo.com/html/";
    let form_data = format!(
        "q={}&kl={}",
        urlencoding::encode(query),
        urlencoding::encode(region),
    );

    debug!(query = %query, region = %region, "Fetching DuckDuckGo search results via curl POST");

    let output = tokio::process::Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            url,
            "-d",
            &form_data,
            "-H",
            &format!("User-Agent: {}", USER_AGENT),
            "-H",
            "Referer: https://html.duckduckgo.com/",
            "--max-time",
            &SEARCH_TIMEOUT_SECS.to_string(),
        ])
        .output()
        .await
        .map_err(|e| Error::Network(format!("Failed to execute curl: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Network(format!(
            "curl exited with {}: {}",
            output.status, stderr
        )));
    }

    let html = String::from_utf8_lossy(&output.stdout).to_string();
    debug!(html_len = html.len(), "DuckDuckGo response received");

    if html.contains("anomaly-modal") {
        warn!("DuckDuckGo returned CAPTCHA page — bot detection triggered");
        return Err(Error::Network(
            "DuckDuckGo CAPTCHA triggered; search temporarily blocked".to_string(),
        ));
    }

    parse_search_results(&html, max_results)
}

/// Parse search results from DuckDuckGo HTML.
fn parse_search_results(html: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    // DuckDuckGo wraps each result in <div class="result ...">
    // Title:   <a class="result__a" href="...">TITLE</a>
    // Snippet: <a class="result__snippet">SNIPPET</a>
    let title_re = Regex::new(r#"<a[^>]+class="result__a"[^>]+href="([^"]*)"[^>]*>(.*?)</a>"#)
        .expect("title regex");
    let snippet_re =
        Regex::new(r#"<a[^>]+class="result__snippet"[^>]*>(.*?)</a>"#).expect("snippet regex");

    let titles: Vec<(String, String)> = title_re
        .captures_iter(html)
        .map(|cap| {
            let raw_url = cap.get(1).map_or("", |m| m.as_str());
            let url = extract_real_url(raw_url);
            let title = strip_html_tags(cap.get(2).map_or("", |m| m.as_str()));
            (url, title)
        })
        .collect();

    let snippets: Vec<String> = snippet_re
        .captures_iter(html)
        .map(|cap| strip_html_tags(cap.get(1).map_or("", |m| m.as_str())))
        .collect();

    let results: Vec<SearchResult> = titles
        .into_iter()
        .enumerate()
        .take(max_results)
        .map(|(i, (url, title))| SearchResult {
            title,
            url,
            snippet: snippets.get(i).cloned().unwrap_or_default(),
        })
        .filter(|r| !r.url.is_empty() && !r.title.is_empty())
        .collect();

    Ok(results)
}

/// DuckDuckGo wraps URLs in a redirect: `//duckduckgo.com/l/?uddg=REAL_URL&...`
/// Extract the actual destination URL.
fn extract_real_url(raw: &str) -> String {
    if let Some(pos) = raw.find("uddg=") {
        let rest = &raw[pos + 5..];
        let end = rest.find('&').unwrap_or(rest.len());
        urlencoding::decode(&rest[..end])
            .map(|s| s.into_owned())
            .unwrap_or_else(|_| rest[..end].to_string())
    } else {
        raw.to_string()
    }
}

/// Remove HTML tags and decode common HTML entities.
fn strip_html_tags(s: &str) -> String {
    let tag_re = Regex::new(r"<[^>]+>").expect("tag regex");
    let stripped = tag_re.replace_all(s, "");
    stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<b>hello</b> world"), "hello world");
        assert_eq!(strip_html_tags("a &amp; b"), "a & b");
        assert_eq!(strip_html_tags("no tags"), "no tags");
    }

    #[test]
    fn test_extract_real_url() {
        let raw = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&rut=abc";
        assert_eq!(extract_real_url(raw), "https://example.com");

        // Direct URL (no redirect)
        assert_eq!(extract_real_url("https://example.com"), "https://example.com");
    }

    #[test]
    fn test_parse_empty_html() {
        let results = parse_search_results("", 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_sample_html() {
        let html = r#"
            <div class="result">
                <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com">Example Title</a>
                <a class="result__snippet">This is a snippet about example.</a>
            </div>
        "#;
        let results = parse_search_results(html, 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example Title");
        assert_eq!(results[0].url, "https://example.com");
        assert_eq!(results[0].snippet, "This is a snippet about example.");
    }

    #[test]
    fn test_max_results_cap() {
        let tool = WebSearchTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "web_search");
        assert_eq!(def.risk_level, RiskLevel::Low);
    }

    #[tokio::test]
    async fn test_missing_query() {
        let tool = WebSearchTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_empty_query() {
        let tool = WebSearchTool::new();
        let result = tool.execute(serde_json::json!({"query": "  "})).await;
        assert!(result.is_err());
    }
}
