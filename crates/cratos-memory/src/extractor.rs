//! Rule-based entity extraction — no LLM calls.
//!
//! Extracts entities from turn content using regex patterns and keyword
//! dictionaries. Each entity gets a relevance score based on position.

use crate::types::{EntityKind, ExtractedEntity, ExtractedRelation, RelationKind};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

// ── Compiled patterns ───────────────────────────────────────────

static RE_ACRONYM: LazyLock<Regex> = LazyLock::new(|| {
    // Uppercase acronyms 2-6 chars (SNS, API, LLM, OAuth, etc.)
    Regex::new(r"\b([A-Z][A-Za-z0-9]{1,5})\b").unwrap()
});

static RE_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([\w\-]+\.(?:rs|toml|json|yaml|yml|md|py|ts|js|tsx|jsx|sql|sh|css|html))\b")
        .unwrap()
});

static RE_FUNCTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pub\s+)?(?:async\s+)?fn\s+(\w+)").unwrap());

static RE_FUNCTION_CALL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\w+)\(\)").unwrap());

static RE_CRATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(cratos-\w+)\b").unwrap());

static RE_ERROR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:error\[E\d+\]|Error::\w+|panic!?\b|unwrap\(\))").unwrap());

/// Technical concept keywords.
static CONCEPT_KEYWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "graph rag",
        "embedding",
        "vector",
        "hnsw",
        "cosine",
        "websocket",
        "socket mode",
        "oauth",
        "jwt",
        "bearer",
        "rate limit",
        "middleware",
        "auth",
        "rbac",
        "scope",
        "sqlite",
        "redis",
        "migration",
        "wal",
        "tokio",
        "async",
        "spawn",
        "channel",
        "onnx",
        "tract",
        "silero",
        "vad",
        "whisper",
        "stt",
        "tts",
        "llm",
        "prompt",
        "completion",
        "tool call",
        "function call",
        "orchestrator",
        "planner",
        "session",
        "context window",
        "replay",
        "event sourcing",
        "audit",
        "telegram",
        "slack",
        "discord",
        "matrix",
        "docker",
        "sandbox",
        "seccomp",
        "mcp",
        "sse",
        "json-rpc",
        "ci/cd",
        "github actions",
        "pull request",
    ]
    .into_iter()
    .collect()
});

/// Known tool names (kept short; callers may supply extras).
static TOOL_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "exec",
        "web_search",
        "read_file",
        "write_file",
        "list_dir",
        "http_request",
        "search",
        "memory",
        "calculator",
        "calendar",
        "reminder",
        "note",
        "code_review",
        "git",
    ]
    .into_iter()
    .collect()
});

/// Result of entity and relation extraction.
#[derive(Debug, Default)]
pub struct ExtractionResult {
    /// Extracted entities.
    pub entities: Vec<ExtractedEntity>,
    /// Extracted relations.
    pub relations: Vec<ExtractedRelation>,
}

/// Extract entities and relations from a turn's content.
pub fn extract(content: &str) -> ExtractionResult {
    let mut seen = HashSet::new();
    let mut entities = Vec::new();
    let mut relations = Vec::new();
    let content_lower = content.to_lowercase();

    // Determine if match is in the first line (higher relevance)
    let first_line_end = content.find('\n').unwrap_or(content.len());

    // ── Files ───────────────────────────────────────────────
    let mut files = Vec::new();
    for cap in RE_FILE.captures_iter(content) {
        let name = cap[1].to_string();
        if seen.insert(("file", name.clone())) {
            let pos = cap.get(0).map(|m| m.start()).unwrap_or(usize::MAX);
            entities.push(ExtractedEntity {
                name: name.clone(),
                kind: EntityKind::File,
                relevance: position_relevance(pos, first_line_end),
            });
            files.push(name);
        }
    }

    // ── Functions ───────────────────────────────────────────
    let mut found_functions = Vec::new();
    for cap in RE_FUNCTION.captures_iter(content) {
        let name = cap[1].to_string();
        if seen.insert(("function", name.clone())) {
            let pos = cap.get(0).map(|m| m.start()).unwrap_or(usize::MAX);
            entities.push(ExtractedEntity {
                name: name.clone(),
                kind: EntityKind::Function,
                relevance: position_relevance(pos, first_line_end),
            });
            found_functions.push(name);
        }
    }

    // ── Crates ──────────────────────────────────────────────
    let mut found_crates = Vec::new();
    for cap in RE_CRATE.captures_iter(content) {
        let name = cap[1].to_string();
        if seen.insert(("crate", name.clone())) {
            let pos = cap.get(0).map(|m| m.start()).unwrap_or(usize::MAX);
            entities.push(ExtractedEntity {
                name: name.clone(),
                kind: EntityKind::Crate,
                relevance: position_relevance(pos, first_line_end),
            });
            found_crates.push(name);
        }
    }

    // ── Function Calls (mentions) ───────────────────────────
    let mut called_functions = Vec::new();
    for cap in RE_FUNCTION_CALL.captures_iter(content) {
        let name = cap[1].to_string();
        // If it's not already seen as a definition in this turn, it's a call/mention
        if !found_functions.contains(&name) && seen.insert(("function_call", name.clone())) {
            entities.push(ExtractedEntity {
                name: name.clone(),
                kind: EntityKind::Function,
                relevance: 0.6,
            });
            called_functions.push(name);
        }
    }

    // ── Relations (Enhanced Heuristics) ─────────────────────

    // 1. File defines/imports
    // If files and functions/crates are mentioned, link them.
    for file_name in &files {
        for func in &found_functions {
            relations.push(ExtractedRelation {
                from_entity: file_name.clone(),
                to_entity: func.clone(),
                kind: RelationKind::Defines,
            });
        }
        for crt in &found_crates {
            relations.push(ExtractedRelation {
                from_entity: file_name.clone(),
                to_entity: crt.clone(),
                kind: RelationKind::Imports,
            });
        }
    }

    // 2. Function calls
    // If a function is defined in this turn and other functions are called,
    // we hypothesize the defined function calls the others.
    // If no function is defined but multiple are called, we link them as Related.
    if !found_functions.is_empty() {
        for defined in &found_functions {
            for called in &called_functions {
                relations.push(ExtractedRelation {
                    from_entity: defined.clone(),
                    to_entity: called.clone(),
                    kind: RelationKind::Calls,
                });
            }
        }
    } else if called_functions.len() >= 2 {
        for i in 0..called_functions.len() {
            for j in (i + 1)..called_functions.len() {
                relations.push(ExtractedRelation {
                    from_entity: called_functions[i].clone(),
                    to_entity: called_functions[j].clone(),
                    kind: RelationKind::Related,
                });
            }
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

    // ── Acronyms (uppercase 2-6 chars: SNS, API, LLM, OAuth, etc.) ──
    for cap in RE_ACRONYM.captures_iter(content) {
        let name = cap[1].to_string();
        let lower = name.to_lowercase();
        // Skip noise words and already-seen entries
        if lower.len() >= 2
            && !["the", "and", "for", "not", "but", "with", "from", "into"]
                .contains(&lower.as_str())
            && seen.insert(("acronym", lower.clone()))
        {
            entities.push(ExtractedEntity {
                name: lower,
                kind: EntityKind::Concept,
                relevance: 0.5,
            });
        }
    }

    ExtractionResult {
        entities,
        relations,
    }
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
        let result = extract("Fix the bug in orchestrator.rs and update Cargo.toml");
        let files: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::File)
            .collect();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|e| e.name == "orchestrator.rs"));
        assert!(files.iter().any(|e| e.name == "Cargo.toml"));
    }

    #[test]
    fn test_extract_functions() {
        let result = extract("The pub async fn process() method handles execution.");
        let funcs: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Function)
            .collect();
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "process");
    }

    #[test]
    fn test_extract_function_calls() {
        let result = extract("fn main() { run_task(); }");
        let funcs: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Function)
            .collect();
        assert_eq!(funcs.len(), 2);
        assert!(funcs.iter().any(|e| e.name == "main"));
        assert!(funcs.iter().any(|e| e.name == "run_task"));

        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == RelationKind::Calls)
            .collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from_entity, "main");
        assert_eq!(calls[0].to_entity, "run_task");
    }

    #[test]
    fn test_extract_crates() {
        let result = extract("cratos-core and cratos-llm need changes");
        let crates: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Crate)
            .collect();
        assert_eq!(crates.len(), 2);
    }

    #[test]
    fn test_extract_errors() {
        let result = extract("Got Error::Config and a panic! in the code");
        let errors: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Error)
            .collect();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_extract_tools() {
        let result = extract("Used exec to run the command and web_search for info");
        let tools: Vec<_> = result
            .entities
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
        let result = extract("Implementing graph rag with embedding search over sqlite");
        let concepts: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Concept)
            .collect();
        assert!(concepts.iter().any(|e| e.name == "graph rag"));
        assert!(concepts.iter().any(|e| e.name == "embedding"));
        assert!(concepts.iter().any(|e| e.name == "sqlite"));
    }

    #[test]
    fn test_first_line_relevance() {
        let result = extract("orchestrator.rs is the target\nAlso check store.rs");
        let files: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::File)
            .collect();
        let orch = files.iter().find(|e| e.name == "orchestrator.rs").unwrap();
        let store = files.iter().find(|e| e.name == "store.rs").unwrap();
        assert!(orch.relevance > store.relevance);
    }

    #[test]
    fn test_dedup() {
        let result = extract("orchestrator.rs and orchestrator.rs again");
        let files: Vec<_> = result
            .entities
            .iter()
            .filter(|e| e.name == "orchestrator.rs")
            .collect();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_empty_content() {
        assert!(extract("").entities.is_empty());
    }
}
