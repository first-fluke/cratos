//! Image Generation Tool - Google Gemini NanoBanana (Imagen 3)
//!
//! Generates images from text descriptions using Google's Generative AI API.
//! Requires `GEMINI_API_KEY` environment variable.

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use reqwest::header::{CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Instant;
use tracing::{debug, warn};

/// Google Gemini API Endpoint Template
/// specific model: NanoBanana (or user-configured)
const GOOGLE_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Request payload for Google Image Generation API (Imagen style)
#[derive(Debug, Serialize)]
struct GoogleImageRequest {
    instances: Vec<GoogleImageInstance>,
    parameters: GoogleImageParameters,
}

#[derive(Debug, Serialize)]
struct GoogleImageInstance {
    prompt: String,
}

#[derive(Debug, Serialize)]
struct GoogleImageParameters {
    #[serde(rename = "sampleCount")]
    sample_count: usize,
    #[serde(rename = "aspectRatio")]
    aspect_ratio: String,
}

/// Response payload from Google Image API
#[derive(Debug, Deserialize)]
struct GoogleImageResponse {
    predictions: Option<Vec<GoogleImagePrediction>>,
    error: Option<GoogleError>,
}

#[derive(Debug, Deserialize)]
struct GoogleImagePrediction {
    #[serde(rename = "bytesBase64Encoded")]
    bytes_base64_encoded: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
}

#[derive(Debug, Deserialize)]
struct GoogleError {
    code: i32,
    message: String,
    status: String,
}

/// Tool for generating images via Google Gemini (NanoBanana)
pub struct ImageGenerationTool {
    definition: ToolDefinition,
}

impl ImageGenerationTool {
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "image_generate",
            "Generate an image from a text description using Google Gemini API (NanoBanana). \
             Returns the base64-encoded image data URI. \
             Use this tool when the user asks to draw, create, or generate an image.",
        )
        .with_category(ToolCategory::External)
        .with_risk_level(RiskLevel::Medium)
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Detailed text description of the image to generate"
                },
                "aspect_ratio": {
                    "type": "string",
                    "enum": ["1:1", "3:4", "4:3", "9:16", "16:9"],
                    "description": "Aspect ratio of the generated image (default: 1:1)"
                }
            },
            "required": ["prompt"]
        }));

        Self { definition }
    }
}

impl Default for ImageGenerationTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for ImageGenerationTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        // Use GEMINI_API_KEY as per user standard for Google models
        let api_key = env::var("GEMINI_API_KEY").map_err(|_| {
            Error::Config("GEMINI_API_KEY environment variable is not set".to_string())
        })?;

        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'prompt' parameter".to_string()))?;

        if prompt.trim().is_empty() {
            return Err(Error::InvalidInput("Prompt must not be empty".to_string()));
        }

        let aspect_ratio = input
            .get("aspect_ratio")
            .and_then(|v| v.as_str())
            .unwrap_or("1:1");

        // User requested "NanoBanana", assuming it maps to a valid model ID or alias.
        // If "NanoBanana" is a custom tuned model, this works.
        // If it's a nickname for Imagen 3, we use "imagen-3.0-generate-001" effectively via config?
        // Given explicit instruction, we target "NanoBanana" unless overridden by env var.
        let model = env::var("CRATOS_IMAGE_MODEL").unwrap_or_else(|_| "NanoBanana".to_string());
        
        // Construct standard Google Generative AI URL: .../models/{model}:predict
        let url = format!("{}/{}:predict?key={}", GOOGLE_API_BASE, model, api_key);

        let request_body = GoogleImageRequest {
            instances: vec![GoogleImageInstance {
                prompt: prompt.to_string(),
            }],
            parameters: GoogleImageParameters {
                sample_count: 1,
                aspect_ratio: aspect_ratio.to_string(),
            },
        };

        debug!(prompt = %prompt, model = %model, "Sending image generation request to Google Gemini API");

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to send request: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %error_text, "Google Image API error");
            return Err(Error::Execution(format!(
                "Google API returned error {}: {}",
                status, error_text
            )));
        }

        let response_body: GoogleImageResponse = response
            .json()
            .await
            .map_err(|e| Error::Execution(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = response_body.error {
            return Err(Error::Execution(format!(
                "Google API Error {}: {}",
                error.code, error.message
            )));
        }

        let predictions = response_body.predictions.ok_or_else(|| {
            Error::Execution("Google API returned no predictions".to_string())
        })?;

        let prediction = predictions.first().ok_or_else(|| {
            Error::Execution("Google API returned empty predictions list".to_string())
        })?;

        let duration = start.elapsed().as_millis() as u64;

        // Return Data URI for immediate display
        let data_uri = format!(
            "data:{};base64,{}",
            prediction.mime_type, prediction.bytes_base64_encoded
        );

        debug!("Image generated successfully (base64)");

        Ok(ToolResult::success(
            serde_json::json!({
                "url": data_uri, // Frontend expects "url", data URI works fine in <img> src
                "provider": "google/NanoBanana",
                "revised_prompt": prompt // Google API doesn't always return revised prompt in prediction
            }),
            duration,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_tool_definition() {
        let tool = ImageGenerationTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "image_generate");
        assert_eq!(def.category, ToolCategory::External);
    }
}
