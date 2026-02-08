//! Local OAuth callback server for browser-based authentication.
//!
//! Starts a temporary HTTP server on `127.0.0.1:<random_port>`,
//! opens the user's browser, and waits for the OAuth callback.

use axum::{extract::Query, extract::State, response::Html, routing::get, Router};
use cratos_llm::oauth::{self, OAuthProviderConfig, OAuthTokens};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::oneshot;

/// Timeout for waiting for the OAuth callback (5 minutes).
const OAUTH_TIMEOUT_SECS: u64 = 300;

/// Shared state for the callback handler.
#[derive(Clone)]
struct CallbackState {
    /// Oneshot sender wrapped to be Clone-compatible for axum handlers.
    /// Sends `Ok(code)` on success or `Err(message)` on failure.
    #[allow(clippy::type_complexity)]
    code_tx: Arc<Mutex<Option<oneshot::Sender<Result<String, String>>>>>,
    /// Expected CSRF state parameter for validation.
    expected_state: String,
}

/// Run a complete OAuth2 PKCE flow: start server, open browser, wait for callback.
///
/// Returns the exchanged `OAuthTokens` on success.
/// Supports manual flow for headless environments via stdin.
pub async fn run_oauth_flow(
    config: &OAuthProviderConfig,
    headless: bool,
    texts: &super::i18n::Texts,
) -> anyhow::Result<OAuthTokens> {
    let pkce = oauth::generate_pkce();
    let csrf_state = oauth::generate_state();

    // Create oneshot channel for the authorization code
    let (code_tx, mut code_rx) = oneshot::channel::<Result<String, String>>();
    let state = CallbackState {
        code_tx: Arc::new(Mutex::new(Some(code_tx))),
        expected_state: csrf_state.clone(),
    };

    // Build callback route
    let redirect_path = config.redirect_path.clone();
    let callback_router = Router::new()
        .route(&redirect_path, get(callback_handler))
        .with_state(state);

    // Bind to random port on localhost
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    // Use 'localhost' instead of '127.0.0.1' as it's more commonly whitelisted for OAuth apps (like gcloud)
    let redirect_uri = format!("http://localhost:{}{}", addr.port(), config.redirect_path);

    // Build authorization URL with CSRF state
    let auth_url = oauth::build_auth_url(config, &redirect_uri, &pkce, &csrf_state);

    // Create shutdown signal for graceful server termination
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Start server in background with graceful shutdown
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, callback_router.into_make_service())
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
    });

    // Handle browser / instructions
    let mut manual_mode = headless;
    if !headless {
        if let Err(e) = try_open_browser(&auth_url) {
            println!("\n  Could not open browser automatically: {}", e);
            manual_mode = true;
        }
    }

    if manual_mode {
        println!("\n  {}", texts.oauth_manual_instructions);
        println!("\n    {}\n", auth_url);
        println!("  {}", texts.oauth_paste_prompt);
    } else {
        println!("  {}", texts.oauth_waiting);
    }

    // Race between server callback and stdin (if manual mode or fallback)
    // Even in non-headless, user might fail to trigger callback and paste manually.
    let code = loop {
        let mut stdin = BufReader::new(tokio::io::stdin());
        let mut line = String::new();

        // We use select! to wait for either callback or stdin
        // Note: checking stdin requires user to press Enter.
        tokio::select! {
            res = &mut code_rx => {
                match res {
                    Ok(Ok(c)) => break c,
                    Ok(Err(e)) => return Err(anyhow::anyhow!("OAuth callback error: {}", e)),
                    Err(_) => return Err(anyhow::anyhow!("OAuth callback channel closed")),
                }
            }
            res = stdin.read_line(&mut line) => {
                // If read_line returns (user pasted something), process it.
                // If it's empty, ignore.
                match res {
                    Ok(0) => continue, // EOF?
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() { continue; }

                        // Try to parse valid URL and extract code + state
                        if let Ok(url) = reqwest::Url::parse(trimmed) {
                            let pairs: HashMap<_, _> = url.query_pairs().collect();
                            if let Some(code_val) = pairs.get("code") {
                                // Verify state if present in the URL
                                if let Some(url_state) = pairs.get("state") {
                                    if url_state.as_ref() != csrf_state {
                                        println!("  CSRF state mismatch. Please try again.");
                                        println!("  {}", texts.oauth_paste_prompt);
                                        continue;
                                    }
                                }
                                break code_val.to_string();
                            }
                        }
                        // Fallback: assume the input IS the code if it looks like one
                        if !trimmed.starts_with("http") && trimmed.len() > 10 {
                             break trimmed.to_string();
                        }

                        println!("  Invalid input. Please paste the full redirected URL.");
                        println!("  {}", texts.oauth_paste_prompt);
                    }
                    Err(_) => continue,
                }
            }
             _ = tokio::time::sleep(std::time::Duration::from_secs(OAUTH_TIMEOUT_SECS)) => {
                 return Err(anyhow::anyhow!("OAuth timeout ({}s)", OAUTH_TIMEOUT_SECS));
             }
        }
    };

    // Exchange code for tokens
    let tokens = oauth::exchange_code(config, &code, &redirect_uri, &pkce.code_verifier).await?;

    // Save tokens
    oauth::save_tokens(&config.token_file, &tokens)?;

    // Graceful shutdown: give the browser time to receive the HTML response
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server_handle).await;

    Ok(tokens)
}

/// Axum handler for the OAuth callback route.
async fn callback_handler(
    State(state): State<CallbackState>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    // Check for OAuth error response
    if let Some(error) = params.get("error") {
        let description = params
            .get("error_description")
            .cloned()
            .unwrap_or_else(|| error.clone());
        if let Some(tx) = state.code_tx.lock().ok().and_then(|mut opt| opt.take()) {
            let _ = tx.send(Err(description.clone()));
        }
        return Html(error_html(&description));
    }

    // Verify CSRF state parameter
    match params.get("state") {
        Some(received_state) if received_state == &state.expected_state => {}
        Some(_) => {
            let msg = "CSRF state mismatch — possible cross-site request forgery".to_string();
            if let Some(tx) = state.code_tx.lock().ok().and_then(|mut opt| opt.take()) {
                let _ = tx.send(Err(msg.clone()));
            }
            return Html(error_html(&msg));
        }
        None => {
            let msg = "Missing state parameter in callback".to_string();
            if let Some(tx) = state.code_tx.lock().ok().and_then(|mut opt| opt.take()) {
                let _ = tx.send(Err(msg.clone()));
            }
            return Html(error_html(&msg));
        }
    }

    // Extract authorization code
    if let Some(code) = params.get("code") {
        if let Some(tx) = state.code_tx.lock().ok().and_then(|mut opt| opt.take()) {
            let _ = tx.send(Ok(code.clone()));
        }
        Html(success_html())
    } else {
        let msg = "Missing authorization code in callback".to_string();
        if let Some(tx) = state.code_tx.lock().ok().and_then(|mut opt| opt.take()) {
            let _ = tx.send(Err(msg.clone()));
        }
        Html(error_html(&msg))
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

/// Try to open a URL in the user's default browser.
fn try_open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "unsupported platform",
        ));
    }
    Ok(())
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
