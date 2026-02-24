use super::adapter::DiscordAdapter;
use super::commands::DiscordCommands;
use crate::util::{mask_for_logging, sanitize_error_for_user, DISCORD_MESSAGE_LIMIT};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cratos_core::{Orchestrator, OrchestratorInput};
use serenity::all::{
    Command, CommandInteraction, CommandOptionType, ComponentInteraction, Context,
    CreateAttachment, CreateCommand, CreateCommandOption, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, EventHandler, Interaction, Message,
    MessageReference, Ready,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{error, info};

/// Discord event handler
pub struct DiscordHandler {
    adapter: Arc<DiscordAdapter>,
    orchestrator: Arc<Orchestrator>,
    commands: DiscordCommands,
}

impl DiscordHandler {
    /// Create a new Discord event handler.
    pub fn new(adapter: Arc<DiscordAdapter>, orchestrator: Arc<Orchestrator>) -> Self {
        Self {
            adapter,
            orchestrator: orchestrator.clone(),
            commands: DiscordCommands::new(orchestrator),
        }
    }
}

#[serenity::async_trait]
impl EventHandler for DiscordHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        let discriminator = ready
            .user
            .discriminator
            .map(|d| format!("#{}", d))
            .unwrap_or_default();
        info!(
            "Discord bot connected as {}{}",
            ready.user.name, discriminator
        );

        // Store bot user ID
        self.adapter
            .bot_user_id
            .store(ready.user.id.get(), Ordering::SeqCst);

        // Register global slash commands
        let commands = vec![
            CreateCommand::new("status").description("Show system status"),
            CreateCommand::new("sessions").description("List active AI sessions"),
            CreateCommand::new("tools").description("List available tools"),
            CreateCommand::new("cancel")
                .description("Cancel an execution")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "id",
                        "Execution ID to cancel",
                    )
                    .required(true),
                ),
            CreateCommand::new("approve")
                .description("Approve a pending request")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "id",
                        "Request ID to approve",
                    )
                    .required(true),
                ),
        ];

        match Command::set_global_commands(&ctx.http, commands).await {
            Ok(cmds) => info!("Registered {} Discord slash commands", cmds.len()),
            Err(e) => error!(error = %e, "Failed to register Discord slash commands"),
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                let response = match command.data.name.as_str() {
                    "status" => {
                        let embed = self.commands.handle_status();
                        CreateInteractionResponseMessage::new().embed(embed)
                    }
                    "sessions" => CreateInteractionResponseMessage::new()
                        .content(self.commands.handle_sessions()),
                    "tools" => CreateInteractionResponseMessage::new()
                        .content(self.commands.handle_tools()),
                    "cancel" => {
                        let id = get_string_option(&command, "id").unwrap_or_default();
                        CreateInteractionResponseMessage::new()
                            .content(self.commands.handle_cancel(&id))
                    }
                    "approve" => {
                        let id = get_string_option(&command, "id").unwrap_or_default();
                        CreateInteractionResponseMessage::new()
                            .content(self.commands.handle_approve(&id).await)
                    }
                    _ => CreateInteractionResponseMessage::new().content("Unknown command"),
                };

                let builder = CreateInteractionResponse::Message(response);
                if let Err(e) = command.create_response(&ctx.http, builder).await {
                    error!(error = %e, "Failed to respond to slash command");
                }
            }
            Interaction::Component(component) => {
                self.handle_component(&ctx, &component).await;
            }
            _ => {}
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let Some(normalized) = self.adapter.normalize_message(&msg) else {
            return;
        };

        // SECURITY: Mask potentially sensitive content in logs
        info!(
            channel_id = %normalized.channel_id,
            user_id = %normalized.user_id,
            text = %mask_for_logging(&normalized.text),
            "Received Discord message"
        );

        // Send typing indicator
        let _ = msg.channel_id.broadcast_typing(&ctx.http).await;

        // Process with orchestrator
        let input = OrchestratorInput::new(
            "discord",
            &normalized.channel_id,
            &normalized.user_id,
            &normalized.text,
        );

        match self.orchestrator.process(input).await {
            Ok(result) => {
                let response_text = if result.response.is_empty() {
                    "I've completed the task.".to_string()
                } else {
                    result.response
                };

                // Discord has a message character limit
                let chunks: Vec<&str> = response_text
                    .as_bytes()
                    .chunks(DISCORD_MESSAGE_LIMIT)
                    .filter_map(|chunk| std::str::from_utf8(chunk).ok())
                    .collect();

                for chunk in chunks {
                    let builder = CreateMessage::new()
                        .content(chunk)
                        .reference_message(MessageReference::from((msg.channel_id, msg.id)));

                    if let Err(e) = msg.channel_id.send_message(&ctx.http, builder).await {
                        error!(error = %e, "Failed to send Discord response");
                    }
                }

                // Handle artifacts (files, images)
                for artifact in &result.artifacts {
                    // Decode artifact data
                    let data = match BASE64.decode(&artifact.data) {
                        Ok(d) => d,
                        Err(e) => {
                            error!(error = %e, artifact = %artifact.name, "Failed to decode artifact data");
                            continue;
                        }
                    };

                    let discord_attachment = CreateAttachment::bytes(data, &artifact.name);
                    let caption = format!("Artifact: {}", artifact.name);

                    let builder = CreateMessage::new()
                        .add_file(discord_attachment)
                        .content(&caption)
                        .reference_message(MessageReference::from((msg.channel_id, msg.id)));

                    if let Err(e) = msg.channel_id.send_message(&ctx.http, builder).await {
                        error!(error = %e, artifact = %artifact.name, "Failed to send Discord artifact");
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to process Discord message");

                let user_message = sanitize_error_for_user(&e.to_string());
                let builder = CreateMessage::new()
                    .content(format!("Sorry, I encountered an error: {}", user_message))
                    .reference_message(MessageReference::from((msg.channel_id, msg.id)));

                let _ = msg.channel_id.send_message(&ctx.http, builder).await;
            }
        }
    }
}

impl DiscordHandler {
    /// Handle button (component) interactions from approval/deny embeds
    async fn handle_component(&self, ctx: &Context, component: &ComponentInteraction) {
        let custom_id = &component.data.custom_id;
        let response_text = if let Some(id) = custom_id.strip_prefix("approve:") {
            self.commands.handle_approve(id).await
        } else if let Some(id) = custom_id.strip_prefix("deny:") {
            self.commands.handle_deny(id).await
        } else {
            "Unknown action.".to_string()
        };

        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(response_text)
                .ephemeral(true),
        );
        if let Err(e) = component.create_response(&ctx.http, builder).await {
            error!(error = %e, "Failed to respond to component interaction");
        }
    }
}

/// Extract a string option from a slash command interaction
fn get_string_option(command: &CommandInteraction, name: &str) -> Option<String> {
    command
        .data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_str().map(|s| s.to_string()))
}
