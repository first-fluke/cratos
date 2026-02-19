use super::*;
use tempfile::TempDir;
use crate::registry::Tool;
use super::types::{ConfigInput, ConfigAction, ConfigTarget};
use crate::registry::RiskLevel;

fn create_test_tool() -> (ConfigTool, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let tool = ConfigTool::with_data_dir(temp_dir.path().to_path_buf());
    (tool, temp_dir)
}

#[tokio::test]
async fn test_wol_registration_without_mac() {
    let (tool, _dir) = create_test_tool();

    let input = serde_json::json!({
        "action": "set",
        "target": "wol_device",
        "device_name": "원격피씨"
    });

    let result = tool.execute(input).await.unwrap();
    let output: serde_json::Value = result.output.clone();

    assert_eq!(output["status"], "needs_info");
    assert!(output["guidance"].as_str().unwrap().contains("MAC"));
}

#[tokio::test]
async fn test_wol_registration_with_mac() {
    let (tool, _dir) = create_test_tool();

    let input = serde_json::json!({
        "action": "set",
        "target": "wol_device",
        "device_name": "원격피씨",
        "mac_address": "AA:BB:CC:DD:EE:FF"
    });

    let result = tool.execute(input).await.unwrap();
    let output: serde_json::Value = result.output.clone();

    assert_eq!(output["status"], "success");
    assert_eq!(output["device_name"], "원격피씨");
}

#[tokio::test]
async fn test_wol_list_empty() {
    let (tool, _dir) = create_test_tool();

    let input = serde_json::json!({
        "action": "list",
        "target": "wol_device"
    });

    let result = tool.execute(input).await.unwrap();
    let output: serde_json::Value = result.output.clone();

    assert_eq!(output["status"], "success");
    assert_eq!(output["devices"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_set_language() {
    let (tool, _dir) = create_test_tool();

    let input = serde_json::json!({
        "action": "set",
        "target": "language",
        "value": "ko"
    });

    let result = tool.execute(input).await.unwrap();
    let output: serde_json::Value = result.output.clone();

    assert_eq!(output["status"], "success");
    assert_eq!(output["value"], "ko");
}

#[tokio::test]
async fn test_set_invalid_value() {
    let (tool, _dir) = create_test_tool();

    let input = serde_json::json!({
        "action": "set",
        "target": "language",
        "value": "invalid_language"
    });

    let result = tool.execute(input).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_language() {
    let (tool, _dir) = create_test_tool();

    // First set
    let set_input = serde_json::json!({
        "action": "set",
        "target": "language",
        "value": "ko"
    });
    tool.execute(set_input).await.unwrap();

    // Then get
    let get_input = serde_json::json!({
        "action": "get",
        "target": "language"
    });

    let result = tool.execute(get_input).await.unwrap();
    let output: serde_json::Value = result.output.clone();

    assert_eq!(output["current_value"], "ko");
}

#[tokio::test]
async fn test_list_options() {
    let (tool, _dir) = create_test_tool();

    let input = serde_json::json!({
        "action": "list",
        "target": "persona"
    });

    let result = tool.execute(input).await.unwrap();
    let output: serde_json::Value = result.output.clone();

    assert_eq!(output["status"], "success");
    let options = output["options"].as_array().unwrap();
    assert!(options.iter().any(|v| v == "cratos"));
    assert!(options.iter().any(|v| v == "sindri"));
}

#[tokio::test]
async fn test_delete_resets_to_default() {
    let (tool, _dir) = create_test_tool();

    // Set a value
    let set_input = serde_json::json!({
        "action": "set",
        "target": "language",
        "value": "ko"
    });
    tool.execute(set_input).await.unwrap();

    // Delete (reset)
    let delete_input = serde_json::json!({
        "action": "delete",
        "target": "language"
    });

    let result = tool.execute(delete_input).await.unwrap();
    let output: serde_json::Value = result.output.clone();

    assert_eq!(output["value"], "en"); // Default
}

#[tokio::test]
async fn test_set_and_get_channel() {
    let (tool, _dir) = create_test_tool();

    let set_input = serde_json::json!({
        "action": "set",
        "target": "channel",
        "value": "slack"
    });
    tool.execute(set_input).await.unwrap();

    let get_input = serde_json::json!({
        "action": "get",
        "target": "channel"
    });
    let result = tool.execute(get_input).await.unwrap();
    let output: serde_json::Value = result.output.clone();
    assert_eq!(output["current_value"], "slack");
}

#[tokio::test]
async fn test_set_and_get_theme() {
    let (tool, _dir) = create_test_tool();

    let set_input = serde_json::json!({
        "action": "set",
        "target": "theme",
        "value": "light"
    });
    tool.execute(set_input).await.unwrap();

    let get_input = serde_json::json!({
        "action": "get",
        "target": "theme"
    });
    let result = tool.execute(get_input).await.unwrap();
    let output: serde_json::Value = result.output.clone();
    assert_eq!(output["current_value"], "light");
}

#[test]
fn test_config_tool_definition() {
    let tool = ConfigTool::new();
    assert_eq!(tool.definition().name, "config");
    assert_eq!(tool.definition().risk_level, RiskLevel::Medium);
}

#[test]
fn test_config_target_options() {
    assert!(!ConfigTarget::LlmProvider.available_options().is_empty());
    assert!(!ConfigTarget::LlmModel.available_options().is_empty());
    assert!(!ConfigTarget::Persona.available_options().is_empty());
}

#[test]
fn test_serde_deserialization() {
    let json = serde_json::json!({
        "action": "set",
        "target": "llm_model",
        "value": "claude-sonnet-4"
    });

    let input: ConfigInput = serde_json::from_value(json).unwrap();
    assert_eq!(input.action, ConfigAction::Set);
    assert_eq!(input.target, ConfigTarget::LlmModel);
    assert_eq!(input.value, Some("claude-sonnet-4".to_string()));
}
