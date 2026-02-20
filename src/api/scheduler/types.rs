use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use cratos_core::scheduler::{ScheduledTask, TaskAction, TriggerType};

/// Task view for API responses
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaskView {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub trigger_type: String,
    pub trigger_config: serde_json::Value,
    pub action_type: String,
    pub action_config: serde_json::Value,
    pub enabled: bool,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub run_count: i64,
    pub failure_count: i64,
}

/// Request to create a new task
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTaskRequest {
    pub name: String,
    pub description: Option<String>,
    pub trigger_type: String,
    pub trigger_config: serde_json::Value,
    pub action_type: String,
    pub action_config: serde_json::Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
}

pub(crate) fn default_true() -> bool {
    true
}

/// Request to update a task
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct UpdateTaskRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub trigger_config: Option<serde_json::Value>,
    pub action_config: Option<serde_json::Value>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
}

/// Convert a ScheduledTask to a TaskView for API response
pub fn task_to_view(task: &ScheduledTask) -> TaskView {
    let (trigger_type, trigger_config) = match &task.trigger {
        TriggerType::Cron(c) => (
            "cron".to_string(),
            serde_json::json!({ "expression": c.expression, "timezone": c.timezone }),
        ),
        TriggerType::Interval(i) => (
            "interval".to_string(),
            serde_json::json!({ "seconds": i.seconds, "immediate": i.immediate }),
        ),
        TriggerType::OneTime(o) => ("one_time".to_string(), serde_json::json!({ "at": o.at })),
        TriggerType::File(f) => (
            "file".to_string(),
            serde_json::to_value(f).unwrap_or_default(),
        ),
        TriggerType::System(s) => (
            "system".to_string(),
            serde_json::to_value(s).unwrap_or_default(),
        ),
    };

    let (action_type, action_config) = match &task.action {
        TaskAction::NaturalLanguage { prompt, channel } => (
            "natural_language".to_string(),
            serde_json::json!({ "prompt": prompt, "channel": channel }),
        ),
        TaskAction::ToolCall { tool, args } => (
            "tool_call".to_string(),
            serde_json::json!({ "tool": tool, "args": args }),
        ),
        TaskAction::Notification {
            channel,
            channel_id,
            message,
        } => (
            "notification".to_string(),
            serde_json::json!({ "channel": channel, "channel_id": channel_id, "message": message }),
        ),
        TaskAction::Shell { command, cwd } => (
            "shell".to_string(),
            serde_json::json!({ "command": command, "cwd": cwd }),
        ),
        TaskAction::Webhook { .. } => (
            "webhook".to_string(),
            serde_json::to_value(&task.action).unwrap_or_default(),
        ),
        TaskAction::RunSkillAnalysis { dry_run } => (
            "run_skill_analysis".to_string(),
            serde_json::json!({ "dry_run": dry_run }),
        ),
        TaskAction::PruneStaleSkills { days } => (
            "prune_stale_skills".to_string(),
            serde_json::json!({ "days": days }),
        ),
    };

    TaskView {
        id: task.id,
        name: task.name.clone(),
        description: task.description.clone(),
        trigger_type,
        trigger_config,
        action_type,
        action_config,
        enabled: task.enabled,
        priority: task.priority,
        created_at: task.created_at,
        last_run_at: task.last_run_at,
        next_run_at: task.next_run_at,
        run_count: task.run_count,
        failure_count: task.failure_count,
    }
}

/// Parse trigger from API request
pub fn parse_trigger(
    trigger_type: &str,
    config: &serde_json::Value,
) -> Result<TriggerType, String> {
    match trigger_type {
        "cron" => {
            let expr = config["expression"]
                .as_str()
                .ok_or("Missing cron expression")?;
            Ok(TriggerType::cron(expr))
        }
        "interval" => {
            let seconds = config["seconds"]
                .as_u64()
                .ok_or("Missing interval seconds")?;
            Ok(TriggerType::interval(seconds))
        }
        "one_time" => {
            let at_str = config["at"].as_str().ok_or("Missing one_time 'at' field")?;
            let at = DateTime::parse_from_rfc3339(at_str)
                .map_err(|e| format!("Invalid datetime: {}", e))?
                .with_timezone(&Utc);
            Ok(TriggerType::one_time(at))
        }
        other => Err(format!("Invalid trigger type: {}", other)),
    }
}

/// Parse action from API request
pub fn parse_action(action_type: &str, config: &serde_json::Value) -> Result<TaskAction, String> {
    match action_type {
        "natural_language" => {
            let prompt = config["prompt"]
                .as_str()
                .ok_or("Missing prompt")?
                .to_string();
            let channel = config["channel"].as_str().map(String::from);
            Ok(TaskAction::NaturalLanguage { prompt, channel })
        }
        "tool_call" => {
            let tool = config["tool"]
                .as_str()
                .ok_or("Missing tool name")?
                .to_string();
            let args = config
                .get("args")
                .cloned()
                .unwrap_or(serde_json::Value::Object(Default::default()));
            Ok(TaskAction::ToolCall { tool, args })
        }
        "notification" => {
            let channel = config["channel"]
                .as_str()
                .ok_or("Missing channel")?
                .to_string();
            let channel_id = config["channel_id"].as_str().unwrap_or("").to_string();
            let message = config["message"]
                .as_str()
                .ok_or("Missing message")?
                .to_string();
            Ok(TaskAction::Notification {
                channel,
                channel_id,
                message,
            })
        }
        "shell" => {
            let command = config["command"]
                .as_str()
                .ok_or("Missing command")?
                .to_string();
            let cwd = config["cwd"].as_str().map(String::from);
            Ok(TaskAction::Shell { command, cwd })
        }
        "webhook" => serde_json::from_value(config.clone()).map_err(|e| e.to_string()),
        "run_skill_analysis" => {
            let dry_run = config["dry_run"].as_bool().unwrap_or(false);
            Ok(TaskAction::RunSkillAnalysis { dry_run })
        }
        "prune_stale_skills" => {
            let days = config["days"].as_u64().unwrap_or(90) as u32;
            Ok(TaskAction::PruneStaleSkills { days })
        }
        other => Err(format!("Invalid action type: {}", other)),
    }
}
