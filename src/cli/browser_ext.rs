//! CLI browser extension commands
//!
//! Provides `cratos browser extension install/path` and
//! `cratos browser tabs/open/screenshot` commands.

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use std::path::PathBuf;

/// Embedded Chrome extension source files.
static CHROME_EXT_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/chrome-extension");

/// Get the extension install path: `~/.cratos/extensions/chrome`
fn extension_path() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .map(|p| p.join(".cratos"))
        .context("Cannot determine home directory")?;
    Ok(data_dir.join("extensions").join("chrome"))
}

/// Install the Chrome extension to `~/.cratos/extensions/chrome`
pub async fn install() -> Result<()> {
    let dest = extension_path()?;
    println!("Installing Chrome extension to: {}", dest.display());

    // Remove old installation if present
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::create_dir_all(&dest)?;

    extract_dir(&CHROME_EXT_DIR, &dest)?;

    println!("Installed successfully.");
    println!();
    println!("To load in Chrome:");
    println!("  1. Open chrome://extensions");
    println!("  2. Enable Developer mode");
    println!("  3. Click 'Load unpacked'");
    println!("  4. Select: {}", dest.display());
    Ok(())
}

/// Recursively extract an `include_dir` tree to disk.
fn extract_dir(dir: &Dir<'_>, dest: &std::path::Path) -> Result<()> {
    for file in dir.files() {
        let path = dest.join(file.path());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, file.contents())?;
    }
    for sub in dir.dirs() {
        extract_dir(sub, dest)?;
    }
    Ok(())
}

/// Print the extension install path.
pub async fn path() -> Result<()> {
    let p = extension_path()?;
    println!("{}", p.display());
    Ok(())
}

/// List browser tabs via REST API.
pub async fn tabs() -> Result<()> {
    let url = server_url("/api/v1/browser/tabs");
    let resp = reqwest::get(&url)
        .await
        .context("Failed to connect to server")?;
    let body: serde_json::Value = resp.json().await?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

/// Open a URL in the browser via REST API.
pub async fn open(url_to_open: &str) -> Result<()> {
    let api_url = server_url("/api/v1/browser/open");
    let client = reqwest::Client::new();
    let resp = client
        .post(&api_url)
        .json(&serde_json::json!({ "url": url_to_open }))
        .send()
        .await
        .context("Failed to connect to server")?;
    let body: serde_json::Value = resp.json().await?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

/// Take a browser screenshot via REST API.
pub async fn screenshot(output: Option<&str>, selector: Option<&str>) -> Result<()> {
    let api_url = server_url("/api/v1/browser/screenshot");
    let client = reqwest::Client::new();
    let mut params = serde_json::json!({});
    if let Some(sel) = selector {
        params["selector"] = serde_json::Value::String(sel.to_string());
    }
    let resp = client
        .post(&api_url)
        .json(&params)
        .send()
        .await
        .context("Failed to connect to server")?;
    let body: serde_json::Value = resp.json().await?;

    let out_path = output.unwrap_or("screenshot.png");

    if let Some(b64) = body.get("screenshot").and_then(|v| v.as_str()) {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
        std::fs::write(out_path, &bytes)?;
        println!("Screenshot saved to: {}", out_path);
    } else {
        println!("{}", serde_json::to_string_pretty(&body)?);
    }
    Ok(())
}

/// Build server URL from env or default.
fn server_url(path: &str) -> String {
    let base =
        std::env::var("CRATOS_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:19527".to_string());
    format!("{}{}", base.trim_end_matches('/'), path)
}
