/// Sanitize Ollama API error messages to prevent leaking sensitive information
pub(crate) fn sanitize_api_error(error: &str) -> String {
    let lower = error.to_lowercase();

    // Don't expose internal paths or system information
    if lower.contains("/home")
        || lower.contains("/root")
        || lower.contains("/var")
        || lower.contains("\\users\\")
    {
        return "An internal error occurred. Please check your Ollama installation.".to_string();
    }

    // For connection errors, provide helpful message
    if lower.contains("connection refused") || lower.contains("failed to connect") {
        return "Failed to connect to Ollama. Is Ollama running?".to_string();
    }

    // For model errors
    if lower.contains("model") && (lower.contains("not found") || lower.contains("pull")) {
        return "Model not available. Please pull the model first with: ollama pull <model>"
            .to_string();
    }

    // Truncate overly long messages but preserve useful error info
    if error.len() > 300 {
        format!("{}...(truncated)", crate::util::truncate_safe(error, 300))
    } else {
        error.to_string()
    }
}
