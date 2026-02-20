use super::types::{parse_action, parse_trigger, task_to_view};
use cratos_core::scheduler::{ScheduledTask, TaskAction, TriggerType};

fn is_valid_trigger_type(trigger_type: &str) -> bool {
    matches!(
        trigger_type,
        "cron" | "interval" | "one_time" | "file" | "system"
    )
}

fn is_valid_action_type(action_type: &str) -> bool {
    matches!(
        action_type,
        "natural_language" | "tool_call" | "notification" | "shell" | "webhook"
    )
}

#[test]
fn test_valid_trigger_types() {
    assert!(is_valid_trigger_type("cron"));
    assert!(is_valid_trigger_type("interval"));
    assert!(is_valid_trigger_type("one_time"));
    assert!(!is_valid_trigger_type("invalid"));
}

#[test]
fn test_valid_action_types() {
    assert!(is_valid_action_type("notification"));
    assert!(is_valid_action_type("natural_language"));
    assert!(is_valid_action_type("tool_call"));
    assert!(!is_valid_action_type("invalid"));
}

#[test]
fn test_parse_trigger_cron() {
    let config = serde_json::json!({"expression": "0 9 * * *"});
    let trigger = parse_trigger("cron", &config).unwrap();
    assert!(matches!(trigger, TriggerType::Cron(_)));
}

#[test]
fn test_parse_trigger_interval() {
    let config = serde_json::json!({"seconds": 3600});
    let trigger = parse_trigger("interval", &config).unwrap();
    assert!(matches!(trigger, TriggerType::Interval(_)));
}

#[test]
fn test_parse_trigger_invalid() {
    let config = serde_json::json!({});
    let result = parse_trigger("invalid", &config);
    assert!(result.is_err());
}

#[test]
fn test_parse_action_natural_language() {
    let config = serde_json::json!({"prompt": "Check status"});
    let action = parse_action("natural_language", &config).unwrap();
    assert!(matches!(action, TaskAction::NaturalLanguage { .. }));
}

#[test]
fn test_parse_action_tool_call() {
    let config = serde_json::json!({"tool": "exec", "args": {"command": "ls"}});
    let action = parse_action("tool_call", &config).unwrap();
    assert!(matches!(action, TaskAction::ToolCall { .. }));
}

#[test]
fn test_parse_action_invalid() {
    let config = serde_json::json!({});
    let result = parse_action("invalid", &config);
    assert!(result.is_err());
}

#[test]
fn test_task_to_view() {
    let task = ScheduledTask::new(
        "test_task",
        TriggerType::interval(3600),
        TaskAction::NaturalLanguage {
            prompt: "Hello".to_string(),
            channel: None,
        },
    );
    let view = task_to_view(&task);
    assert_eq!(view.name, "test_task");
    assert_eq!(view.trigger_type, "interval");
    assert_eq!(view.action_type, "natural_language");
    assert!(view.enabled);
}
