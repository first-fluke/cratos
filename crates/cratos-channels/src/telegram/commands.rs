//! Telegram slash command handlers

use crate::util::sanitize_error_for_user;
use cratos_core::dev_sessions::DevSessionMonitor;
use cratos_core::{Orchestrator, OrchestratorInput};
use std::sync::Arc;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    types::{ChatId, MessageId, ParseMode, ReplyParameters},
};
use tracing::info;

/// Handle a slash command (e.g. /status, /sessions, /tools, /cancel, /approve)
pub async fn handle_slash_command(
    command: &str,
    args: &str,
    orchestrator: &Arc<Orchestrator>,
    dev_monitor: &Option<Arc<DevSessionMonitor>>,
    bot: &Bot,
    chat_id: ChatId,
    reply_to: MessageId,
) -> Option<ResponseResult<()>> {
    info!(
        %command,
        %args,
        %chat_id,
        "Processing slash command"
    );
    let response = match command {
        "/status" => {
            let mut lines = vec!["<b>System Status</b>".to_string()];

            // Active dev sessions
            if let Some(monitor) = dev_monitor {
                let sessions = monitor.sessions().await;
                if sessions.is_empty() {
                    lines.push("AI Sessions: None active".to_string());
                } else {
                    lines.push(format!("AI Sessions: {} active", sessions.len()));
                    for s in &sessions {
                        lines.push(format!(
                            "  - {:?} ({:?}) @ {}",
                            s.tool,
                            s.status,
                            s.project_path.as_deref().unwrap_or("unknown")
                        ));
                    }
                }
            } else {
                lines.push("AI Sessions: Monitor not available".to_string());
            }

            // Active executions
            if let Some(count) = orchestrator.active_execution_count() {
                lines.push(format!("Active executions: {}", count));
            }

            lines.join("\n")
        }
        "/sessions" => {
            if let Some(monitor) = dev_monitor {
                let sessions = monitor.sessions().await;
                if sessions.is_empty() {
                    "No active AI development sessions.".to_string()
                } else {
                    let mut lines = vec![format!("<b>Active AI Sessions ({})</b>", sessions.len())];
                    for (i, s) in sessions.iter().enumerate() {
                        lines.push(format!(
                            "{}. <b>{:?}</b> - {:?}\n   Path: <code>{}</code>\n   PID: {:?}",
                            i + 1,
                            s.tool,
                            s.status,
                            s.project_path.as_deref().unwrap_or("unknown"),
                            s.pid,
                        ));
                    }
                    lines.join("\n")
                }
            } else {
                "DevSessionMonitor not available.".to_string()
            }
        }
        "/tools" => {
            let tool_names = orchestrator.list_tool_names();
            if tool_names.is_empty() {
                "No tools registered.".to_string()
            } else {
                let mut lines = vec![format!("<b>Available Tools ({})</b>", tool_names.len())];
                for name in &tool_names {
                    lines.push(format!("  - <code>{}</code>", name));
                }
                lines.join("\n")
            }
        }
        "/cancel" => {
            if args.is_empty() {
                // No argument: cancel all active executions
                let active = orchestrator.active_executions();
                if active.is_empty() {
                    "No active executions to cancel.".to_string()
                } else {
                    let ids: Vec<uuid::Uuid> = active.iter().map(|e| *e.key()).collect();
                    let mut cancelled = 0;
                    for id in &ids {
                        if orchestrator.cancel_execution(*id) {
                            cancelled += 1;
                        }
                    }
                    format!("Cancelled {} execution(s).", cancelled)
                }
            } else if let Ok(exec_id) = args.parse::<uuid::Uuid>() {
                if orchestrator.cancel_execution(exec_id) {
                    format!("Cancelled execution <code>{}</code>", exec_id)
                } else {
                    format!(
                        "Execution <code>{}</code> not found or already completed.",
                        exec_id
                    )
                }
            } else {
                "Invalid execution ID. Please provide a valid UUID.".to_string()
            }
        }
        "/approve" => {
            if args.is_empty() {
                "Usage: /approve &lt;request_id&gt;".to_string()
            } else {
                // Delegate to orchestrator, which may not have approval manager here
                format!(
                    "Approval for <code>{}</code> — use WebSocket gateway for full approval flow.",
                    args
                )
            }
        }
        "/agent" => {
            if args.is_empty() {
                "Usage: /agent &lt;claude|codex|gemini|antigravity&gt; &lt;prompt&gt;\n\
                 Example: /agent claude Fix the bug in auth.rs"
                    .to_string()
            } else {
                // Parse: first word = agent name, rest = prompt
                let mut agent_parts = args.splitn(2, ' ');
                let agent_name = agent_parts.next().unwrap_or("");
                let agent_prompt = agent_parts.next().unwrap_or("").trim();

                if agent_prompt.is_empty() {
                    format!(
                        "Usage: /agent {} &lt;prompt&gt;\nPlease provide a task description.",
                        agent_name
                    )
                } else {
                    // Delegate to orchestrator as a tool call request
                    let request = format!(
                        "agent_cli 도구를 사용해서 {}에게 다음 작업을 시켜줘: {}",
                        agent_name, agent_prompt
                    );

                    // Return None to let the orchestrator handle this
                    // by falling through to the normal message processing
                    // We rewrite the text so the orchestrator picks it up
                    let chat_id_str = chat_id.0.to_string();
                    let input =
                        OrchestratorInput::new("telegram", &chat_id_str, &chat_id_str, &request);

                    match orchestrator.process(input).await {
                        Ok(result) => {
                            let text = if result.response.is_empty() {
                                format!("Agent '{}' task completed.", agent_name)
                            } else {
                                result.response
                            };
                            crate::util::markdown_to_html(&text)
                        }
                        Err(e) => {
                            format!("Agent error: {}", sanitize_error_for_user(&e.to_string()))
                        }
                    }
                }
            }
        }
        _ => return None,
    };

    let result = bot
        .send_message(chat_id, &response)
        .parse_mode(ParseMode::Html)
        .reply_parameters(ReplyParameters::new(reply_to))
        .await;

    if result.is_err() {
        // Fallback to plain text
        let _ = bot
            .send_message(chat_id, &response)
            .reply_parameters(ReplyParameters::new(reply_to))
            .await;
    }

    Some(Ok(()))
}
