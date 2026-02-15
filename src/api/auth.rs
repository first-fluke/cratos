//! Authentication endpoints for OAuth flows
//!
//! Provides endpoints to initiate OAuth login and handle callbacks.

use axum::{
    extract::Query,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router, Json,
};
use cratos_llm::{
    oauth::{self, PkcePair},
    oauth_config,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex};
use std::sync::LazyLock;

// -----------------------------------------------------------------------------
// State
// -----------------------------------------------------------------------------

struct PendingAuth {
    pkce: PkcePair,
    is_pro: bool,
}

/// Store pending OAuth requests: State string -> PendingAuth
static PENDING_AUTH: LazyLock<Mutex<HashMap<String, PendingAuth>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// -----------------------------------------------------------------------------
// Models
// -----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct LoginParams {
    /// Whether to use Google AI Pro (Gemini CLI) credentials/scopes
    #[serde(default)]
    pro: bool,
    /// Optional redirect URL after success (default: show success message)
    #[allow(dead_code)]
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CallbackParams {
    code: String,
    state: String,
    // Error handling params (e.g. ?error=access_denied)
    #[serde(default)]
    error: Option<String>,
}

// -----------------------------------------------------------------------------
// Routes
// -----------------------------------------------------------------------------

pub fn auth_routes() -> Router {
    Router::new()
        .route("/api/auth/google/login", get(login_google))
        .route("/api/auth/google/callback", get(callback_google))
        .route("/api/auth/google/status", get(status_google))
}

#[derive(Debug, Serialize)]
struct AuthStatusResponse {
    status: String, // "valid", "expired", "not_found"
    is_pro: bool,
}

/// Check Google OAuth authentication status
async fn status_google() -> Json<AuthStatusResponse> {
    use cratos_llm::cli_auth::{check_cratos_google_pro_oauth_status, check_cratos_google_oauth_status, CratosOAuthStatus};

    let pro_status = check_cratos_google_pro_oauth_status();
    let (status, is_pro) = match pro_status {
        CratosOAuthStatus::Valid => ("valid".to_string(), true),
        CratosOAuthStatus::Expired => ("expired".to_string(), true),
        CratosOAuthStatus::NotFound => {
            let std_status = check_cratos_google_oauth_status();
            match std_status {
                CratosOAuthStatus::Valid => ("valid".to_string(), false),
                CratosOAuthStatus::Expired => ("expired".to_string(), false),
                CratosOAuthStatus::NotFound => ("not_found".to_string(), false),
            }
        }
    };

    Json(AuthStatusResponse { status, is_pro })
}

/// Initiate Google OAuth login
async fn login_google(Query(params): Query<LoginParams>) -> Response {
    // 1. Choose configuration
    let config = if params.pro {
        oauth_config::google_pro_oauth_config()
    } else {
        oauth_config::google_oauth_config()
    };

    // 2. Generate PKCE pair and random state
    let pkce = oauth::generate_pkce();
    let state = oauth::generate_state();

    // 3. Store PKCE pair mapped by state
    {
        let mut store = PENDING_AUTH.lock().expect("failed to lock pending auth");
        store.insert(state.clone(), PendingAuth {
            pkce: pkce.clone(),
            is_pro: params.pro,
        });
    }

    // 4. Build Redirect URI
    // Dynamic based on host? For now, we assume local development environment.
    // Ensure this matches the Authorized Redirect URIs in Google Cloud Console.
    let redirect_uri = "http://localhost:19528/api/auth/google/callback";

    // 5. Build Authorization URL
    let url = oauth::build_auth_url(&config, redirect_uri, &pkce, &state);

    // 6. Redirect user (Client-side to avoid proxy following 303)
    let safe_url = url.replace("\"", "&quot;");
    Html(format!(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Redirecting...</title>
    <meta http-equiv="refresh" content="0;url={}">
</head>
<body>
    <p>Redirecting to Google...</p>
    <script>window.location.href = "{}";</script>
</body>
</html>"#, safe_url, url)).into_response()
}

/// Handle Google OAuth callback
async fn callback_google(Query(params): Query<CallbackParams>) -> Response {
    if let Some(error) = params.error {
        return Html(format!("<h1>Authentication Failed</h1><p>Error: {}</p>", error))
            .into_response();
    }

    // 1. Retrieve and remove PendingAuth
    let pending = {
        let mut store = PENDING_AUTH.lock().expect("failed to lock pending auth");
        store.remove(&params.state)
    };

    let pending = match pending {
        Some(p) => p,
        None => {
            return Html("<h1>Invalid Request</h1><p>State parameter mismatch or expired. Please try logging in again.</p>")
                .into_response();
        }
    };

    // 2. Determine configuration based on stored state
    let config = if pending.is_pro {
        oauth_config::google_pro_oauth_config()
    } else {
        oauth_config::google_oauth_config()
    };

    // 3. Exchange code for tokens
    let redirect_uri = "http://localhost:19528/api/auth/google/callback";
    match oauth::exchange_code(
        &config,
        &params.code,
        redirect_uri,
        &pending.pkce.code_verifier,
    )
    .await
    {
        Ok(tokens) => {
            // 4. Save tokens to disk
            // Logic handled by oauth::exchange_code? No, that returns OAuthTokens.
            // We need to save manually using config.token_file.
            if let Err(e) = oauth::save_tokens(&config.token_file, &tokens) {
                return Html(format!("<h1>Token Error</h1><p>Failed to save tokens: {}</p>", e))
                    .into_response();
            }

            Html(r#"
                <html>
                <head><title>Authentication Successful</title></head>
                <body style="font-family: system-ui; text-align: center; padding: 2rem;">
                    <h1>Authentication Successful!</h1>
                    <p>You have successfully logged in via Google.</p>
                    <p>You can close this window now.</p>
                </body>
                </html>
            "#).into_response()
        }
        Err(e) => {
            Html(format!("<h1>Exchange Error</h1><p>Failed to exchange code: {}</p>", e))
                .into_response()
        }
    }
}
