//! Local OAuth callback server for browser-based authentication.
//!
//! Starts a temporary HTTP server on `127.0.0.1:<random_port>`,
//! opens the user's browser, and waits for the OAuth callback.

use axum::{extract::Query, extract::State, response::Html, routing::get, Router};
use cratos_llm::oauth::{self, OAuthProviderConfig, OAuthTokens};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Timeout for waiting for the OAuth callback (5 minutes).
const OAUTH_TIMEOUT_SECS: u64 = 300;

/// Shared state for the callback handler.
#[derive(Clone)]
struct CallbackState {
    /// Oneshot sender wrapped to be Clone-compatible for axum handlers.
    code_tx: Arc<Mutex<Option<oneshot::Sender<String>>>>,
}

/// Run a complete OAuth2 PKCE flow: start server, open browser, wait for callback.
///
/// Returns the exchanged `OAuthTokens` on success.
pub async fn run_oauth_flow(config: &OAuthProviderConfig) -> anyhow::Result<OAuthTokens> {
    let pkce = oauth::generate_pkce();

    // Create oneshot channel for the authorization code
    let (code_tx, code_rx) = oneshot::channel::<String>();
    let state = CallbackState {
        code_tx: Arc::new(Mutex::new(Some(code_tx))),
    };

    // Build callback route
    let redirect_path = config.redirect_path.clone();
    let callback_router = Router::new()
        .route(&redirect_path, get(callback_handler))
        .with_state(state);

    // Bind to random port on localhost
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let redirect_uri = format!("http://127.0.0.1:{}{}", addr.port(), config.redirect_path);

    // Build authorization URL
    let auth_url = oauth::build_auth_url(config, &redirect_uri, &pkce);

    // Start server in background
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, callback_router.into_make_service())
            .await
            .ok();
    });

    // Open browser
    open_browser(&auth_url);

    // Wait for callback with timeout
    let code = tokio::time::timeout(
        std::time::Duration::from_secs(OAUTH_TIMEOUT_SECS),
        code_rx,
    )
    .await
    .map_err(|_| anyhow::anyhow!("OAuth timeout ({}s)", OAUTH_TIMEOUT_SECS))?
    .map_err(|_| anyhow::anyhow!("OAuth callback channel closed"))?;

    // Abort the server
    server_handle.abort();

    // Exchange code for tokens
    let tokens = oauth::exchange_code(config, &code, &redirect_uri, &pkce.code_verifier).await?;

    // Save tokens
    oauth::save_tokens(&config.token_file, &tokens)?;

    Ok(tokens)
}

/// Axum handler for the OAuth callback route.
async fn callback_handler(
    State(state): State<CallbackState>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    if let Some(code) = params.get("code") {
        if let Some(tx) = state.code_tx.lock().ok().and_then(|mut opt| opt.take()) {
            let _ = tx.send(code.clone());
        }
        Html(success_html())
    } else {
        let err = params
            .get("error")
            .cloned()
            .unwrap_or_else(|| "unknown error".to_string());
        Html(error_html(&err))
    }
}

/// Attempt to refresh tokens and save the result.
pub async fn refresh_and_save(
    config: &OAuthProviderConfig,
    refresh_tok: &str,
) -> anyhow::Result<OAuthTokens> {
    let tokens = oauth::refresh_token(config, refresh_tok).await?;
    oauth::save_tokens(&config.token_file, &tokens)?;
    Ok(tokens)
}

/// Open a URL in the user's default browser.
fn open_browser(url: &str) {
    let result = {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open").arg(url).spawn()
        }
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open").arg(url).spawn()
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", url])
                .spawn()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "unsupported platform",
            ))
        }
    };

    match result {
        Ok(_) => {}
        Err(e) => {
            println!();
            println!("  Could not open browser automatically ({})", e);
            println!("  Please open this URL manually:");
            println!();
            println!("    {}", url);
            println!();
        }
    }
}

/// HTML page shown on successful OAuth callback.
fn success_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Cratos</title></head>
<body style="font-family:sans-serif;display:flex;justify-content:center;align-items:center;height:100vh;margin:0;background:#f0f0f0;">
<div style="text-align:center;background:white;padding:3rem;border-radius:12px;box-shadow:0 2px 8px rgba(0,0,0,0.1);">
  <h1 style="color:#22c55e;">Login Successful!</h1>
  <p>You can close this tab and return to your terminal.</p>
</div>
</body>
</html>"#
        .to_string()
}

/// HTML page shown on OAuth error.
fn error_html(error: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Cratos</title></head>
<body style="font-family:sans-serif;display:flex;justify-content:center;align-items:center;height:100vh;margin:0;background:#f0f0f0;">
<div style="text-align:center;background:white;padding:3rem;border-radius:12px;box-shadow:0 2px 8px rgba(0,0,0,0.1);">
  <h1 style="color:#ef4444;">Login Failed</h1>
  <p>Error: {}</p>
  <p>Please close this tab and try again in your terminal.</p>
</div>
</body>
</html>"#,
        error
    )
}
