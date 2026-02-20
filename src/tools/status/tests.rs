use super::*;
use cratos_skills::SkillStore;
use serde_json::json;

#[tokio::test]
async fn test_status_tool_definition() -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(SkillStore::in_memory().await?);
    let tool = StatusTool::new(store);

    let def = tool.definition();
    assert_eq!(def.name, "status");
    assert_eq!(def.risk_level, RiskLevel::Low);
    assert_eq!(def.category, ToolCategory::Utility);
    Ok(())
}

#[tokio::test]
async fn test_status_tool_query_skill_empty() -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(SkillStore::in_memory().await?);
    let tool = StatusTool::new(store);

    let result = tool.execute(json!({"target": "skill"})).await?;

    assert!(result.success);
    let skills = result.output.get("skills").ok_or("Expected skills field")?;
    assert!(skills.is_array());
    assert_eq!(skills.as_array().ok_or("Expected skills array")?.len(), 0);
    Ok(())
}

#[tokio::test]
async fn test_status_tool_query_skill_by_name_not_found() -> Result<(), Box<dyn std::error::Error>>
{
    let store = Arc::new(SkillStore::in_memory().await?);
    let tool = StatusTool::new(store);

    let result = tool
        .execute(json!({"target": "skill", "name": "nonexistent"}))
        .await?;

    assert!(result.success);
    assert!(result.output.get("error").is_some());
    Ok(())
}

#[tokio::test]
async fn test_status_tool_query_all() -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(SkillStore::in_memory().await?);
    let tool = StatusTool::new(store);

    let result = tool.execute(json!({"target": "all"})).await?;

    assert!(result.success);
    assert!(result.output.get("personas").is_some());
    assert!(result.output.get("skills").is_some());
    Ok(())
}

#[tokio::test]
async fn test_status_tool_unknown_target() -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(SkillStore::in_memory().await?);
    let tool = StatusTool::new(store);

    let result = tool.execute(json!({"target": "unknown"})).await?;

    assert!(result.success);
    assert!(result.output.get("error").is_some());
    Ok(())
}
