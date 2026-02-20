use super::*;
use serde_json::json;

async fn make_tool() -> Result<MemoryTool, Box<dyn std::error::Error>> {
    let gm = cratos_memory::GraphMemory::in_memory().await?;
    Ok(MemoryTool::new(Arc::new(gm)))
}

#[tokio::test]
async fn test_memory_tool_definition() -> Result<(), Box<dyn std::error::Error>> {
    let tool = make_tool().await?;
    let def = tool.definition();
    assert_eq!(def.name, "memory");
    assert_eq!(def.risk_level, RiskLevel::Low);
    Ok(())
}

#[tokio::test]
async fn test_save_and_recall() -> Result<(), Box<dyn std::error::Error>> {
    let tool = make_tool().await?;

    // Save
    let result = tool
        .execute(json!({
            "action": "save",
            "name": "api-secret",
            "content": "The API key for service X is abc123",
            "category": "knowledge",
            "tags": ["api", "secret"]
        }))
        .await?;
    assert!(result.success);
    assert_eq!(result.output["status"], "saved");

    // Recall
    let result = tool
        .execute(json!({
            "action": "recall",
            "query": "api-secret"
        }))
        .await?;
    assert!(result.success);
    assert_eq!(result.output["status"], "found");
    assert_eq!(result.output["count"], 1);
    Ok(())
}

#[tokio::test]
async fn test_list_memories() -> Result<(), Box<dyn std::error::Error>> {
    let tool = make_tool().await?;

    tool.execute(json!({
        "action": "save",
        "name": "note-1",
        "content": "First note"
    }))
    .await?;

    let result = tool.execute(json!({"action": "list"})).await?;
    assert!(result.success);
    assert_eq!(result.output["count"], 1);
    Ok(())
}

#[tokio::test]
async fn test_delete_memory() -> Result<(), Box<dyn std::error::Error>> {
    let tool = make_tool().await?;

    tool.execute(json!({
        "action": "save",
        "name": "temp",
        "content": "temporary"
    }))
    .await?;

    let result = tool
        .execute(json!({"action": "delete", "name": "temp"}))
        .await?;
    assert_eq!(result.output["status"], "deleted");

    // Delete again → not_found
    let result = tool
        .execute(json!({"action": "delete", "name": "temp"}))
        .await?;
    assert_eq!(result.output["status"], "not_found");
    Ok(())
}

#[tokio::test]
async fn test_update_memory() -> Result<(), Box<dyn std::error::Error>> {
    let tool = make_tool().await?;

    tool.execute(json!({
        "action": "save",
        "name": "evolving",
        "content": "version 1"
    }))
    .await?;

    let result = tool
        .execute(json!({
            "action": "update",
            "name": "evolving",
            "content": "version 2"
        }))
        .await?;
    assert_eq!(result.output["status"], "updated");

    // Recall and verify content
    let result = tool
        .execute(json!({"action": "recall", "query": "evolving"}))
        .await?;
    let mems = result.output["memories"]
        .as_array()
        .ok_or("Expected memories array")?;
    assert!(mems.iter().any(|m| m["content"] == "version 2"));
    Ok(())
}

#[tokio::test]
async fn test_recall_empty() -> Result<(), Box<dyn std::error::Error>> {
    let tool = make_tool().await?;
    let result = tool
        .execute(json!({"action": "recall", "query": "nonexistent"}))
        .await?;
    assert_eq!(result.output["status"], "no_results");
    Ok(())
}

#[tokio::test]
async fn test_missing_required_fields() -> Result<(), Box<dyn std::error::Error>> {
    let tool = make_tool().await?;

    // Save without name
    let r = tool
        .execute(json!({"action": "save", "content": "x"}))
        .await?;
    assert!(r.output.get("error").is_some());

    // Save without content
    let r = tool.execute(json!({"action": "save", "name": "x"})).await?;
    assert!(r.output.get("error").is_some());

    // Recall without query
    let r = tool.execute(json!({"action": "recall"})).await?;
    assert!(r.output.get("error").is_some());
    Ok(())
}
