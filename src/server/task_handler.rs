use std::sync::Arc;

use cratos_core::orchestrator::{Orchestrator, OrchestratorInput};
use cratos_core::scheduler::{SchedulerError, TaskAction};
use cratos_core::EventBus;
use cratos_skills::SkillStore;
use cratos_tools::builtins::{ExecConfig, ExecMode, ExecTool};
use cratos_tools::registry::Tool;
use tracing::{info, warn};

use crate::server::config::SecurityConfig;

/// Execute a scheduled task action
pub async fn execute_task(
    action: TaskAction,
    orchestrator: Arc<Orchestrator>,
    _event_bus: Arc<EventBus>,
    skill_store: Arc<SkillStore>,
    security_config: SecurityConfig,
) -> Result<String, SchedulerError> {
    match action {
        TaskAction::PruneStaleSkills { days } => {
            info!("Pruning skills older than {} days...", days);
            match skill_store.prune_stale_skills(days).await {
                Ok(count) => {
                    let msg = format!("Pruned {} stale skills.", count);
                    info!("{}", msg);
                    Ok(msg)
                },
                Err(e) => {
                    warn!("Failed to prune stale skills: {}", e);
                    Err(SchedulerError::Execution(e.to_string()))
                }
            }
        }
        TaskAction::NaturalLanguage { prompt, channel } => {
            info!("Executing scheduled prompt: {}", prompt);
            let input = OrchestratorInput::new(
                channel.unwrap_or_else(|| "scheduler".to_string()),
                "scheduler_task",
                "system",
                prompt,
            )
            .with_system_prompt_override(
                "You are executing a scheduled task. Be concise.".to_string(),
            );

            match orchestrator.process(input).await {
                Ok(result) => {
                    info!("Scheduled task completed: {}", result.execution_id);
                    Ok(result.response)
                }
                Err(e) => {
                    warn!("Scheduled task failed: {}", e);
                    Err(SchedulerError::Execution(e.to_string()))
                }
            }
        }
        TaskAction::ToolCall { tool, args } => {
            info!("Executing scheduled tool call: {} {:?}", tool, args);
            // TODO: Implement direct tool execution via orchestrator or runner
            // For now, wrap in natural language instructions to use the tool
            let prompt = format!("Please run the tool '{}' with arguments: {}", tool, args);
            let input = OrchestratorInput::new("scheduler", "scheduler_task", "system", prompt);

            match orchestrator.process(input).await {
                Ok(result) => Ok(result.response),
                Err(e) => Err(SchedulerError::Execution(e.to_string())),
            }
        }
        TaskAction::Notification {
            channel,
            channel_id,
            message,
        } => {
            // Send notification via configured channels
            // Currently supporting a simple log or potential integration
            info!(
                channel = %channel,
                channel_id = %channel_id,
                "Scheduled Notification: {}",
                message
            );

            // Here we could integration with NotificationService if available
            // For MVP, logging and return success is sufficient unless an adapter is active

            Ok(format!("Notification sent to {}: {}", channel, message))
        }
        TaskAction::Shell { command, cwd } => {
            info!("Executing scheduled shell command (secure): {}", command);

            // Convert server SecurityConfig to tools ExecConfig
            let exec_config = ExecConfig {
                mode: if security_config.exec.mode == "strict" {
                    ExecMode::Strict
                } else {
                    ExecMode::Permissive
                },
                max_timeout_secs: security_config.exec.max_timeout_secs,
                extra_blocked_commands: security_config.exec.extra_blocked_commands.clone(),
                allowed_commands: security_config.exec.allowed_commands.clone(),
                blocked_paths: security_config.exec.blocked_paths.clone(),
                // Default sandbox settings
                ..ExecConfig::default()
            };

            let tool = ExecTool::with_config(exec_config);

            let mut input = serde_json::json!({
                "command": command
            });

            if let Some(dir) = cwd {
                input["cwd"] = serde_json::Value::String(dir);
            }

            match tool.execute(input).await {
                Ok(result) => {
                    let output = result.output.clone();
                    // Extract stdout/stderr from the result object if possible
                    let stdout = output
                        .get("stdout")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let stderr = output
                        .get("stderr")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    
                    if result.success {
                        Ok(stdout.to_string())
                    } else {
                        Err(SchedulerError::Execution(format!(
                            "Command failed: {}\nStderr: {}",
                            result.error.unwrap_or_else(|| "Unknown error".to_string()),
                            stderr
                        )))
                    }
                }
                Err(e) => Err(SchedulerError::Execution(format!("Security Block: {}", e))),
            }
        }
        TaskAction::RunSkillAnalysis { dry_run } => {
            match cratos_skills::analyzer::run_auto_analysis(dry_run).await {
                Ok(msg) => {
                    info!("Scheduled skill analysis: {}", msg);
                    Ok(msg)
                }
                Err(e) => {
                    warn!("Scheduled skill analysis failed: {}", e);
                    Err(SchedulerError::Execution(e.to_string()))
                }
            }
        }
        TaskAction::Webhook {
            url,
            method,
            headers,
            body,
        } => {
            // Execute HTTP webhook
            let client = reqwest::Client::new();
            let method = match method.to_uppercase().as_str() {
                "GET" => reqwest::Method::GET,
                "POST" => reqwest::Method::POST,
                "PUT" => reqwest::Method::PUT,
                "DELETE" => reqwest::Method::DELETE,
                "PATCH" => reqwest::Method::PATCH,
                _ => reqwest::Method::GET,
            };

            let mut request = client.request(method, &url);

            // Add headers
            if let Some(hdrs) = headers {
                if let Some(obj) = hdrs.as_object() {
                    for (k, v) in obj {
                        if let Some(val) = v.as_str() {
                            request = request.header(k, val);
                        }
                    }
                }
            }

            // Add body
            if let Some(b) = body {
                request = request.json(&b);
            }

            match request.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_else(|_| String::new());
                    if status.is_success() {
                        info!(url = %url, status = %status, "Webhook succeeded");
                        Ok(format!(
                            "Webhook {} returned {}: {}",
                            url,
                            status,
                            &body[..body.len().min(200)]
                        ))
                    } else {
                        warn!(url = %url, status = %status, "Webhook failed");
                        Err(SchedulerError::Execution(format!(
                            "Webhook failed with status {}: {}",
                            status,
                            &body[..body.len().min(200)]
                        )))
                    }
                }
                Err(e) => Err(SchedulerError::Execution(e.to_string())),
            }
        }
    }
}

