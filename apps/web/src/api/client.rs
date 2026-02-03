//! HTTP API Client

use gloo_net::http::Request;
use serde::{de::DeserializeOwned, Serialize};

/// API client for backend communication
pub struct ApiClient {
    base_url: String,
}

impl ApiClient {
    /// Create a new API client
    pub fn new() -> Self {
        // Get base URL from window location or use default
        let base_url = web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:8080".to_string());

        Self { base_url }
    }

    /// Make a GET request
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);

        Request::get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<T>()
            .await
            .map_err(|e| e.to_string())
    }

    /// Make a POST request
    pub async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);

        Request::post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(body).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<T>()
            .await
            .map_err(|e| e.to_string())
    }

    /// Make a PUT request
    pub async fn put<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);

        Request::put(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(body).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<T>()
            .await
            .map_err(|e| e.to_string())
    }

    /// Make a DELETE request
    pub async fn delete(&self, path: &str) -> Result<(), String> {
        let url = format!("{}{}", self.base_url, path);

        Request::delete(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}
