//! Memory tool — lets the LLM save and recall explicit knowledge
//!
//! Registered as a built-in tool so users can say "기억해줘" or "그때 그거 뭐였지?"
//! and have knowledge persisted across sessions.

use cratos_memory::GraphMemory;
use cratos_tools::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

/// The `memory` tool for saving and recalling explicit knowledge.
pub struct MemoryTool {
    definition: ToolDefinition,
    graph_memory: Arc<GraphMemory>,
}

impl MemoryTool {
    /// Create a new memory tool backed by GraphMemory.
    pub fn new(graph_memory: Arc<GraphMemory>) -> Self {
        let definition = ToolDefinition::new(
            "memory",
            "Save and recall explicit knowledge across sessions. Actions: \
             save (store new knowledge), recall (search saved memories), \
             list (show all saved), update (modify existing), delete (remove).",
        )
        .with_parameters(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["save", "recall", "list", "update", "delete"],
                    "description": "The action to perform"
                },
                "name": {
                    "type": "string",
                    "description": "Memory name (required for save/update/delete)"
                },
                "content": {
                    "type": "string",
                    "description": "Memory content (required for save, optional for update)"
                },
                "category": {
                    "type": "string",
                    "description": "Category: general, knowledge, blueprint, strategy, pattern, error_fix"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Searchable tags"
                },
                "query": {
                    "type": "string",
                    "description": "Search query (required for recall)"
                }
            },
            "required": ["action"]
        }))
        .with_risk_level(RiskLevel::Low)
        .with_category(ToolCategory::Utility);

        Self {
            definition,
            graph_memory,
        }
    }
}

#[async_trait::async_trait]
impl Tool for MemoryTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> cratos_tools::Result<ToolResult> {
        let start = Instant::now();

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("recall");

        let output = match action {
            "save" => {
                let name = match input.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return Ok(ToolResult::success(
                            json!({"error": "name is required for save"}),
                            start.elapsed().as_millis() as u64,
                        ));
                    }
                };
                let content = match input.get("content").and_then(|v| v.as_str()) {
                    Some(c) => c,
                    None => {
                        return Ok(ToolResult::success(
                            json!({"error": "content is required for save"}),
                            start.elapsed().as_millis() as u64,
                        ));
                    }
                };
                let category = input
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("general");
                let tags: Vec<String> = input
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                match self
                    .graph_memory
                    .save_memory(name, content, category, &tags)
                    .await
                {
                    Ok(id) => json!({
                        "status": "saved",
                        "id": id,
                        "name": name,
                        "category": category,
                        "tags": tags,
                    }),
                    Err(e) => json!({"error": format!("save failed: {e}")}),
                }
            }

            "recall" => {
                let query = match input.get("query").and_then(|v| v.as_str()) {
                    Some(q) => q,
                    None => {
                        return Ok(ToolResult::success(
                            json!({"error": "query is required for recall"}),
                            start.elapsed().as_millis() as u64,
                        ));
                    }
                };

                match self.graph_memory.recall_memories(query, 5).await {
                    Ok(memories) if memories.is_empty() => {
                        json!({"status": "no_results", "query": query})
                    }
                    Ok(memories) => {
                        let items: Vec<serde_json::Value> = memories
                            .iter()
                            .map(|m| {
                                json!({
                                    "name": m.name,
                                    "content": m.content,
                                    "category": m.category,
                                    "tags": m.tags,
                                    "access_count": m.access_count,
                                })
                            })
                            .collect();
                        json!({"status": "found", "count": items.len(), "memories": items})
                    }
                    Err(e) => json!({"error": format!("recall failed: {e}")}),
                }
            }

            "list" => {
                let category = input.get("category").and_then(|v| v.as_str());
                match self.graph_memory.list_memories(category, 20).await {
                    Ok(memories) => {
                        let items: Vec<serde_json::Value> = memories
                            .iter()
                            .map(|m| {
                                json!({
                                    "name": m.name,
                                    "category": m.category,
                                    "tags": m.tags,
                                    "access_count": m.access_count,
                                })
                            })
                            .collect();
                        json!({"status": "ok", "count": items.len(), "memories": items})
                    }
                    Err(e) => json!({"error": format!("list failed: {e}")}),
                }
            }

            "update" => {
                let name = match input.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return Ok(ToolResult::success(
                            json!({"error": "name is required for update"}),
                            start.elapsed().as_millis() as u64,
                        ));
                    }
                };
                let content = input.get("content").and_then(|v| v.as_str());
                let category = input.get("category").and_then(|v| v.as_str());
                let tags: Option<Vec<String>> =
                    input.get("tags").and_then(|v| v.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    });

                match self
                    .graph_memory
                    .update_memory(name, content, category, tags.as_deref())
                    .await
                {
                    Ok(()) => json!({"status": "updated", "name": name}),
                    Err(e) => json!({"error": format!("update failed: {e}")}),
                }
            }

            "delete" => {
                let name = match input.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return Ok(ToolResult::success(
                            json!({"error": "name is required for delete"}),
                            start.elapsed().as_millis() as u64,
                        ));
                    }
                };

                match self.graph_memory.delete_memory(name).await {
                    Ok(true) => json!({"status": "deleted", "name": name}),
                    Ok(false) => json!({"status": "not_found", "name": name}),
                    Err(e) => json!({"error": format!("delete failed: {e}")}),
                }
            }

            other => {
                json!({"error": format!("unknown action: '{}'. Use save, recall, list, update, or delete", other)})
            }
        };

        let duration = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(output, duration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_tool() -> MemoryTool {
        let gm = GraphMemory::in_memory().await.unwrap();
        MemoryTool::new(Arc::new(gm))
    }

    #[tokio::test]
    async fn test_memory_tool_definition() {
        let tool = make_tool().await;
        let def = tool.definition();
        assert_eq!(def.name, "memory");
        assert_eq!(def.risk_level, RiskLevel::Low);
    }

    #[tokio::test]
    async fn test_save_and_recall() {
        let tool = make_tool().await;

        // Save
        let result = tool
            .execute(json!({
                "action": "save",
                "name": "api-secret",
                "content": "The API key for service X is abc123",
                "category": "knowledge",
                "tags": ["api", "secret"]
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.output["status"], "saved");

        // Recall
        let result = tool
            .execute(json!({
                "action": "recall",
                "query": "api-secret"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.output["status"], "found");
        assert_eq!(result.output["count"], 1);
    }

    #[tokio::test]
    async fn test_list_memories() {
        let tool = make_tool().await;

        tool.execute(json!({
            "action": "save",
            "name": "note-1",
            "content": "First note"
        }))
        .await
        .unwrap();

        let result = tool.execute(json!({"action": "list"})).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output["count"], 1);
    }

    #[tokio::test]
    async fn test_delete_memory() {
        let tool = make_tool().await;

        tool.execute(json!({
            "action": "save",
            "name": "temp",
            "content": "temporary"
        }))
        .await
        .unwrap();

        let result = tool
            .execute(json!({"action": "delete", "name": "temp"}))
            .await
            .unwrap();
        assert_eq!(result.output["status"], "deleted");

        // Delete again → not_found
        let result = tool
            .execute(json!({"action": "delete", "name": "temp"}))
            .await
            .unwrap();
        assert_eq!(result.output["status"], "not_found");
    }

    #[tokio::test]
    async fn test_update_memory() {
        let tool = make_tool().await;

        tool.execute(json!({
            "action": "save",
            "name": "evolving",
            "content": "version 1"
        }))
        .await
        .unwrap();

        let result = tool
            .execute(json!({
                "action": "update",
                "name": "evolving",
                "content": "version 2"
            }))
            .await
            .unwrap();
        assert_eq!(result.output["status"], "updated");

        // Recall and verify content
        let result = tool
            .execute(json!({"action": "recall", "query": "evolving"}))
            .await
            .unwrap();
        let mems = result.output["memories"].as_array().unwrap();
        assert!(mems.iter().any(|m| m["content"] == "version 2"));
    }

    #[tokio::test]
    async fn test_recall_empty() {
        let tool = make_tool().await;
        let result = tool
            .execute(json!({"action": "recall", "query": "nonexistent"}))
            .await
            .unwrap();
        assert_eq!(result.output["status"], "no_results");
    }

    #[tokio::test]
    async fn test_missing_required_fields() {
        let tool = make_tool().await;

        // Save without name
        let r = tool
            .execute(json!({"action": "save", "content": "x"}))
            .await
            .unwrap();
        assert!(r.output.get("error").is_some());

        // Save without content
        let r = tool
            .execute(json!({"action": "save", "name": "x"}))
            .await
            .unwrap();
        assert!(r.output.get("error").is_some());

        // Recall without query
        let r = tool.execute(json!({"action": "recall"})).await.unwrap();
        assert!(r.output.get("error").is_some());
    }
}
