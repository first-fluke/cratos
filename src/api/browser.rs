//! Browser REST API endpoints.
//!
//! Provides HTTP endpoints for browser extension status and control.

use axum::{
    routing::{get, post},
    Extension, Json, Router,
};
use serde_json::Value;
use std::sync::Arc;

use crate::websocket::gateway::browser_relay::BrowserRelay;

/// Create the browser API routes.
pub fn browser_routes() -> Router {
    Router::new()
        .route("/api/v1/browser/status", get(status))
        .route("/api/v1/browser/tabs", get(tabs))
        .route("/api/v1/browser/open", post(open))
        .route("/api/v1/browser/screenshot", post(screenshot))
        .route("/api/v1/browser/action", post(action))
}

async fn status(Extension(relay): Extension<Arc<BrowserRelay>>) -> Json<Value> {
    let connected = relay.is_connected().await;
    Json(serde_json::json!({ "connected": connected }))
}

async fn tabs(Extension(relay): Extension<Arc<BrowserRelay>>) -> Json<Value> {
    match relay.send_request("browser.get_tabs", Value::Null).await {
        Ok(result) => Json(result),
        Err(msg) => Json(serde_json::json!({ "error": msg })),
    }
}

async fn open(
    Extension(relay): Extension<Arc<BrowserRelay>>,
    Json(params): Json<Value>,
) -> Json<Value> {
    match relay.send_request("browser.navigate", params).await {
        Ok(result) => Json(result),
        Err(msg) => Json(serde_json::json!({ "error": msg })),
    }
}

async fn screenshot(
    Extension(relay): Extension<Arc<BrowserRelay>>,
    Json(params): Json<Value>,
) -> Json<Value> {
    match relay.send_request("browser.screenshot", params).await {
        Ok(result) => Json(result),
        Err(msg) => Json(serde_json::json!({ "error": msg })),
    }
}

async fn action(
    Extension(relay): Extension<Arc<BrowserRelay>>,
    Json(params): Json<Value>,
) -> Json<Value> {
    match relay.send_request("browser.exec_action", params).await {
        Ok(result) => Json(result),
        Err(msg) => Json(serde_json::json!({ "error": msg })),
    }
}
