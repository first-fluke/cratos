//! Rule-based entity extraction — no LLM calls.
//!
//! Extracts entities from turn content using regex patterns and keyword
//! dictionaries. Each entity gets a relevance score based on position.

use crate::types::{EntityKind, ExtractedEntity};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

// ── Compiled patterns ───────────────────────────────────────────

static RE_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([\w\-]+\.(?:rs|toml|json|yaml|yml|md|py|ts|js|tsx|jsx|sql|sh|css|html))\b")
        .unwrap()
});

static RE_FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:pub\s+)?(?:async\s+)?fn\s+(\w+)").unwrap()
});

static RE_CRATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(cratos-\w+)\b").unwrap()
});

static RE_ERROR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:error\[E\d+\]|Error::\w+|panic!?\b|unwrap\(\))").unwrap()
});

// Config key pattern reserved for future use (too many false positives now)
// static RE_CONFIG: LazyLock<Regex> = LazyLock::new(|| {
//     Regex::new(r"\b(\w+(?:\.\w+){1,3})\b").unwrap()
// });

/// Technical concept keywords.
static CONCEPT_KEYWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "graph rag", "embedding", "vector", "hnsw", "cosine",
        "websocket", "socket mode", "oauth", "jwt", "bearer",
        "rate limit", "middleware", "auth", "rbac", "scope",
        "sqlite", "redis", "migration", "wal",
        "tokio", "async", "spawn", "channel",
        "onnx", "tract", "silero", "vad", "whisper", "stt", "tts",
        "llm", "prompt", "completion", "tool call", "function call",
        "orchestrator", "planner", "session", "context window",
        "replay", "event sourcing", "audit",
        "telegram", "slack", "discord", "matrix",
        "docker", "sandbox", "seccomp",
        "mcp", "sse", "json-rpc",
        "ci/cd", "github actions", "pull request",
    ]
    .into_iter()
    .collect()
});

/// Known tool names (kept short; callers may supply extras).
static TOOL_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "exec", "web_search", "read_file", "write_file", "list_dir",
        "http_request", "search", "memory", "calculator", "calendar",
        "reminder", "note", "code_review", "git",
    ]
    .into_iter()
    .collect()
});

/// Extract entities from a turn's content.
pub fn extract(content: &str) -> Vec<ExtractedEntity> {
    let mut seen = HashSet::new();
    let mut entities = Vec::new();
    let content_lower = content.to_lowercase();

    // Determine if match is in the first line (higher relevance)
    let first_line_end = content.find('\n').unwrap_or(content.len());

    // ── Files ───────────────────────────────────────────────
    for cap in RE_FILE.captures_iter(content) {
        let name = cap[1].to_string();
        if seen.insert(("file", name.clone())) {
            let pos = cap.get(0).map(|m| m.start()).unwrap_or(usize::MAX);
            entities.push(ExtractedEntity {
                name,
                kind: EntityKind::File,
                relevance: position_relevance(pos, first_line_end),
            });
        }
    }

    // ── Functions ───────────────────────────────────────────
    for cap in RE_FUNCTION.captures_iter(content) {
        let name = cap[1].to_string();
        if seen.insert(("function", name.clone())) {
            let pos = cap.get(0).map(|m| m.start()).unwrap_or(usize::MAX);
            entities.push(ExtractedEntity {
                name,
                kind: EntityKind::Function,
                relevance: position_relevance(pos, first_line_end),
            });
        }
    }

    // ── Crates ──────────────────────────────────────────────
    for cap in RE_CRATE.captures_iter(content) {
        let name = cap[1].to_string();
        if seen.insert(("crate", name.clone())) {
            let pos = cap.get(0).map(|m| m.start()).unwrap_or(usize::MAX);
            entities.push(ExtractedEntity {
                name,
                kind: EntityKind::Crate,
                relevance: position_relevance(pos, first_line_end),
            });
        }
    }

    // ── Errors ──────────────────────────────────────────────
    for mat in RE_ERROR.find_iter(content) {
        let name = mat.as_str().to_string();
        if seen.insert(("error", name.clone())) {
            entities.push(ExtractedEntity {
                name,
                kind: EntityKind::Error,
                relevance: position_relevance(mat.start(), first_line_end),
            });
        }
    }

    // ── Tools ───────────────────────────────────────────────
    for tool in TOOL_NAMES.iter() {
        if content_lower.contains(tool) && seen.insert(("tool", (*tool).to_string())) {
            entities.push(ExtractedEntity {
                name: (*tool).to_string(),
                kind: EntityKind::Tool,
                relevance: 0.7,
            });
        }
    }

    // ── Concepts ────────────────────────────────────────────
    for keyword in CONCEPT_KEYWORDS.iter() {
        if content_lower.contains(keyword) && seen.insert(("concept", (*keyword).to_string())) {
            entities.push(ExtractedEntity {
                name: (*keyword).to_string(),
                kind: EntityKind::Concept,
                relevance: 0.6,
            });
        }
    }

    entities
}

/// Relevance based on position: first line → 1.0, later → 0.7, code blocks → 0.5.
fn position_relevance(byte_pos: usize, first_line_end: usize) -> f32 {
    if byte_pos <= first_line_end {
        1.0
    } else {
        0.7
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_files() {
        let entities = extract("Fix the bug in orchestrator.rs and update Cargo.toml");
        let files: Vec<_> = entities
            .iter()
            .filter(|e| e.kind == EntityKind::File)
            .collect();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|e| e.name == "orchestrator.rs"));
        assert!(files.iter().any(|e| e.name == "Cargo.toml"));
    }

    #[test]
    fn test_extract_functions() {
        let entities = extract("The pub async fn process() method handles execution.");
        let funcs: Vec<_> = entities
            .iter()
            .filter(|e| e.kind == EntityKind::Function)
            .collect();
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "process");
    }

    #[test]
    fn test_extract_crates() {
        let entities = extract("cratos-core and cratos-llm need changes");
        let crates: Vec<_> = entities
            .iter()
            .filter(|e| e.kind == EntityKind::Crate)
            .collect();
        assert_eq!(crates.len(), 2);
    }

    #[test]
    fn test_extract_errors() {
        let entities = extract("Got Error::Config and a panic! in the code");
        let errors: Vec<_> = entities
            .iter()
            .filter(|e| e.kind == EntityKind::Error)
            .collect();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_extract_tools() {
        let entities = extract("Used exec to run the command and web_search for info");
        let tools: Vec<_> = entities
            .iter()
            .filter(|e| e.kind == EntityKind::Tool)
            .collect();
        // exec, web_search, search (substring of web_search)
        assert!(tools.len() >= 2);
        assert!(tools.iter().any(|e| e.name == "exec"));
        assert!(tools.iter().any(|e| e.name == "web_search"));
    }

    #[test]
    fn test_extract_concepts() {
        let entities = extract("Implementing graph rag with embedding search over sqlite");
        let concepts: Vec<_> = entities
            .iter()
            .filter(|e| e.kind == EntityKind::Concept)
            .collect();
        assert!(concepts.iter().any(|e| e.name == "graph rag"));
        assert!(concepts.iter().any(|e| e.name == "embedding"));
        assert!(concepts.iter().any(|e| e.name == "sqlite"));
    }

    #[test]
    fn test_first_line_relevance() {
        let entities = extract("orchestrator.rs is the target\nAlso check store.rs");
        let files: Vec<_> = entities
            .iter()
            .filter(|e| e.kind == EntityKind::File)
            .collect();
        let orch = files.iter().find(|e| e.name == "orchestrator.rs").unwrap();
        let store = files.iter().find(|e| e.name == "store.rs").unwrap();
        assert!(orch.relevance > store.relevance);
    }

    #[test]
    fn test_dedup() {
        let entities = extract("orchestrator.rs and orchestrator.rs again");
        let files: Vec<_> = entities
            .iter()
            .filter(|e| e.name == "orchestrator.rs")
            .collect();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_empty_content() {
        assert!(extract("").is_empty());
    }
}
