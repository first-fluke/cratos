//! A2UI Security Policy and Validation
//!
//! This module implements security policies for A2UI messages, preventing XSS and enforcing resource limits.

use super::protocol::{A2uiComponentType, A2uiServerMessage};
use serde_json::Value;
use std::collections::HashSet;
use thiserror::Error;

/// A2UI Security Policy configuration
#[derive(Debug, Clone)]
pub struct A2uiSecurityPolicy {
    /// List of allowed component types
    allowed_components: HashSet<A2uiComponentType>,

    /// Allowed domains for navigation (whitelist)
    allowed_domains: Vec<String>,

    /// Whether snapshots/screenshots are allowed
    allow_snapshots: bool,

    /// Whether iframes are allowed
    allow_iframes: bool,

    /// Maximum number of components per message
    pub max_components: usize,

    /// Maximum message size in bytes
    pub max_message_size: usize,

    /// Maximum nesting depth for props
    pub max_props_depth: usize,

    /// Pre-compiled XSS detection regex
    #[doc(hidden)]
    xss_regex: regex::Regex,
}

#[derive(Debug, Error)]
pub enum A2uiSecurityError {
    /// Component not in whitelist
    #[error("component not allowed: {0:?}")]
    ComponentNotAllowed(A2uiComponentType),

    /// Iframes not allowed
    #[error("iframes not allowed")]
    IframesNotAllowed,

    /// Snapshots not allowed
    #[error("snapshots not allowed")]
    SnapshotsNotAllowed,

    /// Possible XSS attempt detected
    #[error("XSS attempt: {0}")]
    XssAttempt(String),

    /// Domain not in whitelist
    #[error("domain not allowed: {0}")]
    DomainNotAllowed(String),

    /// Unsafe URL scheme (javascript:, vbscript:, data:)
    #[error("unsafe scheme: {0}")]
    UnsafeScheme(String),

    /// Malformed URL
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// Message size exceeds limit
    #[error("message too large: {0} bytes")]
    MessageTooLarge(usize),

    /// Props nesting too deep
    #[error("props too deep: {0} levels")]
    PropsTooDeep(usize),

    /// JSON parsing error
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl A2uiSecurityPolicy {
    /// Create a restrictive default policy
    ///
    /// Allows only safe text/presentation components. Blocks JS, iframes, snapshots.
    pub fn default_restrictive() -> Self {
        let allowed = vec![
            A2uiComponentType::Text,
            A2uiComponentType::Markdown,
            A2uiComponentType::Button,
            A2uiComponentType::TextInput,
            A2uiComponentType::Select,
            A2uiComponentType::Card,
            A2uiComponentType::Table,
            A2uiComponentType::Divider,
        ];

        // Match inline handlers like 'onload=', 'onclick='
        let xss_regex = regex::Regex::new(r"(?i)\bon\w+\s*=").expect("Invalid regex");

        Self {
            allowed_components: allowed.into_iter().collect(),
            allowed_domains: vec!["localhost".into()],
            allow_snapshots: false,
            allow_iframes: false,
            max_components: 100,
            max_message_size: 1024 * 1024, // 1MB
            max_props_depth: 10,
            xss_regex,
        }
    }

    /// Create an extended policy with more features enabled
    ///
    /// Allows charts, code blocks, images, modals, grids. Enables snapshots.
    pub fn extended() -> Self {
        let mut policy = Self::default_restrictive();
        policy.allowed_components.extend(vec![
            A2uiComponentType::Chart,
            A2uiComponentType::Code,
            A2uiComponentType::Image,
            A2uiComponentType::Modal,
            A2uiComponentType::Form,
            A2uiComponentType::Grid,
            A2uiComponentType::Tabs,
        ]);
        policy.allow_snapshots = true;
        policy
    }

    /// Validate an incoming server message against the policy
    ///
    /// Checks:
    /// - Component type whitelist
    /// - Prop depth and size limits
    /// - XSS patterns in string props
    /// - URL safety (scheme and domain whitelist)
    pub fn validate(&self, msg: &A2uiServerMessage) -> Result<(), A2uiSecurityError> {
        match msg {
            A2uiServerMessage::Render {
                component_type,
                props,
                ..
            } => {
                self.validate_component(component_type)?;
                self.validate_props(props, 0)?;
                Ok(())
            }
            A2uiServerMessage::Navigate { url, options: _ } => {
                self.validate_url(url)?;
                Ok(())
            }
            A2uiServerMessage::Snapshot { .. } => {
                if !self.allow_snapshots {
                    return Err(A2uiSecurityError::SnapshotsNotAllowed);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn validate_component(
        &self,
        component_type: &A2uiComponentType,
    ) -> Result<(), A2uiSecurityError> {
        if !self.allowed_components.contains(component_type) {
            return Err(A2uiSecurityError::ComponentNotAllowed(
                component_type.clone(),
            ));
        }

        // iframe 특별 처리
        if *component_type == A2uiComponentType::Iframe && !self.allow_iframes {
            return Err(A2uiSecurityError::IframesNotAllowed);
        }

        Ok(())
    }

    fn validate_props(
        &self,
        props: &serde_json::Value,
        depth: usize,
    ) -> Result<(), A2uiSecurityError> {
        // 깊이 제한
        if depth > self.max_props_depth {
            return Err(A2uiSecurityError::PropsTooDeep(depth));
        }

        match props {
            Value::String(s) => {
                self.validate_string_content(s)?;
                Ok(())
            }
            Value::Array(arr) => {
                for v in arr {
                    self.validate_props(v, depth + 1)?;
                }
                Ok(())
            }
            Value::Object(obj) => {
                // 크기 확인 (간단한 추정)
                let size = serde_json::to_vec(obj)?.len();
                if size > self.max_message_size {
                    return Err(A2uiSecurityError::MessageTooLarge(size));
                }

                for v in obj.values() {
                    self.validate_props(v, depth + 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn validate_string_content(&self, s: &str) -> Result<(), A2uiSecurityError> {
        let lower = s.to_lowercase();

        // script 태그
        if lower.contains("<script") || lower.contains("</script") {
            return Err(A2uiSecurityError::XssAttempt("script tag detected".into()));
        }

        // javascript: URL
        if lower.contains("javascript:") {
            return Err(A2uiSecurityError::XssAttempt(
                "javascript URL detected".into(),
            ));
        }

        // data: URL (이미지 제외)
        if lower.contains("data:") && !lower.contains("data:image/") {
            return Err(A2uiSecurityError::XssAttempt(
                "non-image data URL detected".into(),
            ));
        }

        // on* 이벤트 핸들러 (간단한 정규식)
        if self.xss_regex.is_match(s) {
            return Err(A2uiSecurityError::XssAttempt(
                "inline event handler detected".into(),
            ));
        }

        Ok(())
    }

    fn validate_url(&self, url: &str) -> Result<(), A2uiSecurityError> {
        let parsed =
            url::Url::parse(url).map_err(|e| A2uiSecurityError::InvalidUrl(e.to_string()))?;

        // 스킴 확인
        match parsed.scheme() {
            "http" | "https" => {}
            "javascript" => return Err(A2uiSecurityError::UnsafeScheme("javascript".into())),
            "data" => return Err(A2uiSecurityError::UnsafeScheme("data".into())),
            "vbscript" => return Err(A2uiSecurityError::UnsafeScheme("vbscript".into())),
            other => return Err(A2uiSecurityError::UnsafeScheme(other.into())),
        }

        // 도메인 화이트리스트
        let host = parsed.host_str().unwrap_or("");
        let allowed = self
            .allowed_domains
            .iter()
            .any(|d| host == *d || host.ends_with(&format!(".{}", d)));

        if !allowed {
            return Err(A2uiSecurityError::DomainNotAllowed(host.into()));
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "security_tests.rs"]
mod tests;
