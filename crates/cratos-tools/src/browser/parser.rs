use super::actions::BrowserAction;
use super::tool::BrowserTool;
use crate::error::{Error, Result};

impl BrowserTool {
    /// Parse input JSON into a BrowserAction
    pub(super) fn parse_action(&self, input: &serde_json::Value) -> Result<BrowserAction> {
        let action_str = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'action' parameter".to_string()))?;

        match action_str {
            "navigate" => {
                let url = input
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'url' for navigate".to_string()))?;
                Ok(BrowserAction::Navigate {
                    url: url.to_string(),
                    wait_until_loaded: input
                        .get("wait_until_loaded")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                })
            }
            "click" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for click".to_string())
                    })?;
                Ok(BrowserAction::Click {
                    selector: selector.to_string(),
                    button: input
                        .get("button")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                })
            }
            "type" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for type".to_string())
                    })?;
                let text = input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'text' for type".to_string()))?;
                Ok(BrowserAction::Type {
                    selector: selector.to_string(),
                    text: text.to_string(),
                    delay: input.get("delay").and_then(|v| v.as_u64()),
                })
            }
            "fill" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for fill".to_string())
                    })?;
                let value = input
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'value' for fill".to_string()))?;
                Ok(BrowserAction::Fill {
                    selector: selector.to_string(),
                    value: value.to_string(),
                })
            }
            "screenshot" => Ok(BrowserAction::Screenshot {
                path: input.get("path").and_then(|v| v.as_str()).map(String::from),
                full_page: input
                    .get("full_page")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                selector: input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            }),
            "get_text" => Ok(BrowserAction::GetText {
                selector: input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            }),
            "get_html" => Ok(BrowserAction::GetHtml {
                selector: input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                outer: input.get("outer").and_then(|v| v.as_bool()).unwrap_or(true),
            }),
            "get_attribute" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for get_attribute".to_string())
                    })?;
                let attribute =
                    input
                        .get("attribute")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::InvalidInput("Missing 'attribute' for get_attribute".to_string())
                        })?;
                Ok(BrowserAction::GetAttribute {
                    selector: selector.to_string(),
                    attribute: attribute.to_string(),
                })
            }
            "wait_for_selector" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for wait_for_selector".to_string())
                    })?;
                Ok(BrowserAction::WaitForSelector {
                    selector: selector.to_string(),
                    timeout: input
                        .get("timeout")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(crate::browser::DEFAULT_BROWSER_TIMEOUT_MS),
                    visible: input
                        .get("visible")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                })
            }
            "wait_for_navigation" => Ok(BrowserAction::WaitForNavigation {
                timeout: input
                    .get("timeout")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(crate::browser::DEFAULT_BROWSER_TIMEOUT_MS),
            }),
            "evaluate" => {
                let script = input
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'script' for evaluate".to_string())
                    })?;
                Ok(BrowserAction::Evaluate {
                    script: script.to_string(),
                })
            }
            "select" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for select".to_string())
                    })?;
                let value = input
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'value' for select".to_string()))?;
                Ok(BrowserAction::Select {
                    selector: selector.to_string(),
                    value: value.to_string(),
                })
            }
            "check" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for check".to_string())
                    })?;
                Ok(BrowserAction::Check {
                    selector: selector.to_string(),
                    checked: input
                        .get("checked")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                })
            }
            "hover" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for hover".to_string())
                    })?;
                Ok(BrowserAction::Hover {
                    selector: selector.to_string(),
                })
            }
            "press" => {
                let key = input
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'key' for press".to_string()))?;
                Ok(BrowserAction::Press {
                    key: key.to_string(),
                    count: input.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u32,
                })
            }
            "scroll" => Ok(BrowserAction::Scroll {
                selector: input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                x: input.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                y: input.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            }),
            "get_url" => Ok(BrowserAction::GetUrl),
            "get_title" => Ok(BrowserAction::GetTitle),
            "go_back" => Ok(BrowserAction::GoBack),
            "go_forward" => Ok(BrowserAction::GoForward),
            "reload" => Ok(BrowserAction::Reload),
            "resize" => Ok(BrowserAction::Resize {
                width: input.get("width").and_then(|v| v.as_u64()).unwrap_or(1280) as u32,
                height: input.get("height").and_then(|v| v.as_u64()).unwrap_or(720) as u32,
            }),
            "search" => {
                let site = input
                    .get("site")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'site' for search".to_string())
                    })?;
                let query = input
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'query' for search".to_string())
                    })?;
                Ok(BrowserAction::Search {
                    site: site.to_string(),
                    query: query.to_string(),
                })
            }
            "click_text" => {
                let text = input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'text' for click_text".to_string())
                    })?;
                Ok(BrowserAction::ClickText {
                    text: text.to_string(),
                    index: input
                        .get("index")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                })
            }
            "get_tabs" => Ok(BrowserAction::GetTabs),
            "close" => Ok(BrowserAction::Close),
            _ => Err(Error::InvalidInput(format!(
                "Unknown action: {}",
                action_str
            ))),
        }
    }
}
