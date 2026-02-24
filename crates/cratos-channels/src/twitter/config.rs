use crate::error::{Error, Result};
use serde::Deserialize;

/// Twitter API v2 configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct TwitterConfig {
    /// Twitter API key (consumer key).
    pub api_key: String,
    /// Twitter API secret (consumer secret).
    pub api_secret: String,
    /// OAuth access token.
    pub access_token: String,
    /// OAuth access token secret.
    pub access_token_secret: String,
    /// Optional bearer token for app-only authentication.
    pub bearer_token: Option<String>,
    /// Usernames allowed to interact (empty = all allowed).
    pub allowed_users: Vec<String>,
}

impl TwitterConfig {
    /// Create a new config with required OAuth credentials.
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        access_token: impl Into<String>,
        access_token_secret: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            access_token: access_token.into(),
            access_token_secret: access_token_secret.into(),
            bearer_token: None,
            allowed_users: Vec::new(),
        }
    }

    /// Create config from TWITTER_* environment variables.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("TWITTER_API_KEY")
            .map_err(|_| Error::Config("TWITTER_API_KEY not set".to_string()))?;
        let api_secret = std::env::var("TWITTER_API_SECRET")
            .map_err(|_| Error::Config("TWITTER_API_SECRET not set".to_string()))?;
        let access_token = std::env::var("TWITTER_ACCESS_TOKEN")
            .map_err(|_| Error::Config("TWITTER_ACCESS_TOKEN not set".to_string()))?;
        let access_token_secret = std::env::var("TWITTER_ACCESS_TOKEN_SECRET")
            .map_err(|_| Error::Config("TWITTER_ACCESS_TOKEN_SECRET not set".to_string()))?;

        let bearer_token = std::env::var("TWITTER_BEARER_TOKEN").ok();

        let allowed_users = std::env::var("TWITTER_ALLOWED_USERS")
            .ok()
            .map(|s| s.split(',').map(|u| u.trim().to_string()).collect())
            .unwrap_or_default();

        Ok(Self {
            api_key,
            api_secret,
            access_token,
            access_token_secret,
            bearer_token,
            allowed_users,
        })
    }

    /// Set the bearer token for app-only authentication.
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Set the list of allowed usernames.
    pub fn with_allowed_users(mut self, users: Vec<String>) -> Self {
        self.allowed_users = users;
        self
    }
}
