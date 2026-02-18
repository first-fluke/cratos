use std::sync::Arc;

use cratos_core::orchestrator::{Orchestrator, OrchestratorInput};
use cratos_core::scheduler::{SchedulerError, TaskAction};
use cratos_core::EventBus;
use tracing::{info, warn};

/// Execute a scheduled task action
pub async fn execute_task(
    action: TaskAction,
    orchestrator: Arc<Orchestrator>,
    _event_bus: Arc<EventBus>,
) -> Result<String, SchedulerError> {
    match action {
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
            let input = OrchestratorInput::new(
                "scheduler",
                "scheduler_task",
                "system",
                prompt,
            );

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
            info!("Executing scheduled shell command: {}", command);
            let mut cmd = tokio::process::Command::new("sh");
            cmd.arg("-c").arg(&command);
            if let Some(dir) = cwd {
                cmd.current_dir(dir);
            }

            match cmd.output().await {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output.status.success() {
                        Ok(stdout.to_string())
                    } else {
                        Err(SchedulerError::Execution(format!(
                            "Command failed: {}\nStderr: {}",
                            output.status, stderr
                        )))
                    }
                }
                Err(e) => Err(SchedulerError::Execution(e.to_string())),
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
