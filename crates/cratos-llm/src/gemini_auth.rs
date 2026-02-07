use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;
use std::env;

/// Credentials extracted from the local Gemini CLI installation.
#[derive(Debug, Clone)]
pub struct GeminiCliCredentials {
    /// The Google OAuth Client ID
    pub client_id: String,
    /// The Google OAuth Client Secret
    pub client_secret: String,
}

/// Attempts to find and extract Google OAuth credentials from a local Gemini CLI installation.
///
/// This function searches for the `gemini` executable in the system PATH, resolves it to the
/// installation directory, and then looks for the `oauth2.js` file which contains the
/// client ID and secret used by the CLI.
///
/// Returns `Some(GeminiCliCredentials)` if successful, `None` otherwise.
pub fn resolve_gemini_cli_credentials() -> Option<GeminiCliCredentials> {
    if let Some(path) = find_gemini_path() {
        if let Some(creds) = extract_credentials_from_path(&path) {
            return Some(creds);
        }
    }
    None
}

fn find_gemini_path() -> Option<PathBuf> {
    // 1. Try env var override
    if let Ok(p) = env::var("GEMINI_CLI_PATH") {
        return Some(PathBuf::from(p));
    }
    
    // 2. Try PATH
    if let Ok(paths) = env::var("PATH") {
        for path in env::split_paths(&paths) {
            let exe = path.join("gemini");
            if exe.exists() {
                // Return the *real* path (resolve symlinks) to find parallel node_modules
                if let Ok(real) = fs::canonicalize(&exe) {
                    return Some(real);
                }
                return Some(exe);
            }
        }
    }
    
    // 3. Common locations
    let common = vec![
        "/opt/homebrew/bin/gemini",
        "/usr/local/bin/gemini",
        // Add more if needed
    ];
    for p in common {
        let path = PathBuf::from(p);
        if path.exists() {
             if let Ok(real) = fs::canonicalize(&path) {
                return Some(real);
            }
            return Some(path);
        }
    }

    None
}

fn extract_credentials_from_path(gemini_bin_path: &Path) -> Option<GeminiCliCredentials> {
    // gemini_bin_path is typically .../lib/node_modules/@google/gemini-cli/bin/run.js
    // or .../bin/gemini (symlink)
    
    // If it's a symlink resolution (e.g. from homebrew), we might be at:
    // /opt/homebrew/Cellar/node/../lib/node_modules/@google/gemini-cli/bin/run
    
    // We want to find the package root based on this.
    // Assuming standard structure:
    // .../package-name/bin/executable -> .../package-name/
    
    let current = gemini_bin_path.parent()?;
    
    // Naive traverse up to find node_modules structure or known path
    // OpenClaw strategy: dirname(dirname(resolvedPath)) -> geminiCliDir
    if let Some(p) = current.parent() {
        let gemini_cli_dir = p;
        
         // 4. Search for oahtu2.js in known locations relative to package root
        let search_paths = vec![
            // Direct dependency structure
            gemini_cli_dir.join("node_modules/@google/gemini-cli-core/dist/src/code_assist/oauth2.js"),
            gemini_cli_dir.join("node_modules/@google/gemini-cli-core/dist/code_assist/oauth2.js"),
            // Nested dependency (npm/pnpm structure might vary)
            gemini_cli_dir.join("node_modules/@google/gemini-cli/node_modules/@google/gemini-cli-core/dist/code_assist/oauth2.js"),
            // Global install structure (often flatter or different)
            gemini_cli_dir.join("../gemini-cli-core/dist/code_assist/oauth2.js"),
        ];

        for path in &search_paths {
            if path.exists() {
                if let Some(creds) = extract_credentials_from_file(path) {
                    return Some(creds);
                }
            }
        }
        
        // Fallback: try searching recursively a bit? or just assume standard.
        // Let's rely on standard for now.
    }
    
    None
}


fn extract_credentials_from_file(path: &Path) -> Option<GeminiCliCredentials> {
    let content = fs::read_to_string(path).ok()?;
    
    // Regex based on OpenClaw's implementation
    // client_id: /(\d+-[a-z0-9]+\.apps\.googleusercontent\.com)/
    // client_secret: /(GOCSPX-[A-Za-z0-9_-]+)/
    
    let id_regex = Regex::new(r"(\d+-[a-z0-9]+\.apps\.googleusercontent\.com)").ok()?;
    // Secret regex from OpenClaw is just `(GOCSPX-[A-Za-z0-9_-]+)`, verifying if that's robust
    let secret_regex = Regex::new(r"(GOCSPX-[A-Za-z0-9_-]+)").ok()?;
    
    let client_id = id_regex.captures(&content)?.get(1)?.as_str().to_string();
    let client_secret = secret_regex.captures(&content)?.get(1)?.as_str().to_string();
    
    Some(GeminiCliCredentials {
        client_id,
        client_secret,
    })
}
