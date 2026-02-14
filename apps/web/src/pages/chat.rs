//! Chat Page
//!
//! Real-time chat with AI via WebSocket /ws/chat.

use leptos::*;
use leptos_router::*;
use uuid::Uuid;

use crate::api::websocket::{use_chat_websocket, ChatClientMessage, ChatServerMessage, WsState};
use crate::components::MessageBubble;

/// Chat page with real WebSocket connection
#[component]
pub fn Chat() -> impl IntoView {
    let params = use_params_map();

    // Get session ID from URL or create new
    let session_id = move || {
        params.with(|p| {
            p.get("session_id")
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or_else(Uuid::new_v4)
        })
    };

    // Chat state
    let (messages, set_messages) = create_signal::<Vec<ChatMessage>>(Vec::new());
    let (input, set_input) = create_signal(String::new());
    let (is_streaming, set_is_streaming) = create_signal(false);
    let (_current_streaming_id, set_current_streaming_id) = create_signal::<Option<Uuid>>(None);

    // WebSocket connection
    let (ws_state, last_message, send_message) = use_chat_websocket();

    // Handle incoming WebSocket messages
    create_effect(move |_| {
        if let Some(msg) = last_message.get() {
            match msg {
                ChatServerMessage::Connected { session_id } => {
                    gloo_console::log!("Connected with session:", session_id.to_string());
                }
                ChatServerMessage::ChatResponse {
                    execution_id,
                    text,
                    is_final,
                    persona,
                } => {
                    if is_final {
                        // Final response - add as new message or update streaming message
                        set_messages.update(|msgs| {
                            // Check if we're updating a streaming message
                            if let Some(idx) = msgs.iter().position(|m| m.id == execution_id) {
                                msgs[idx].content = text.clone();
                                msgs[idx].blocks = vec![ContentBlock::Markdown(text)];
                            } else {
                                msgs.push(ChatMessage {
                                    id: execution_id,
                                    role: MessageRole::Assistant,
                                    content: text.clone(),
                                    blocks: vec![ContentBlock::Markdown(text)],
                                    persona: Some(persona),
                                });
                            }
                        });
                        set_is_streaming.set(false);
                        set_current_streaming_id.set(None);
                    } else {
                        // Streaming delta - accumulate text
                        let eid = execution_id;
                        set_messages.update(|msgs| {
                            if let Some(idx) = msgs.iter().position(|m| m.id == eid) {
                                msgs[idx].content.push_str(&text);
                            } else {
                                // First chunk - create new message
                                msgs.push(ChatMessage {
                                    id: eid,
                                    role: MessageRole::Assistant,
                                    content: text.clone(),
                                    blocks: vec![],
                                    persona: Some(persona),
                                });
                            }
                        });
                        set_current_streaming_id.set(Some(eid));
                    }
                }
                ChatServerMessage::ToolCall {
                    execution_id,
                    tool_name,
                    status,
                } => {
                    gloo_console::log!(
                        "Tool:",
                        tool_name.clone(),
                        status.clone(),
                        execution_id.to_string()
                    );
                    // Could show tool activity in UI
                }
                ChatServerMessage::Artifact {
                    execution_id,
                    filename,
                    mime_type,
                    data,
                } => {
                    gloo_console::log!("Artifact received:", filename.clone());
                    // Could display artifact in UI
                    set_messages.update(|msgs| {
                        if let Some(idx) = msgs.iter().position(|m| m.id == execution_id) {
                            if mime_type.starts_with("image/") {
                                msgs[idx].blocks.push(ContentBlock::Image {
                                    url: format!("data:{};base64,{}", mime_type, data),
                                    alt: filename,
                                });
                            }
                        }
                    });
                }
                ChatServerMessage::Error { message, code } => {
                    gloo_console::error!("Error:", message.clone(), code.clone());
                    set_messages.update(|msgs| {
                        msgs.push(ChatMessage {
                            id: Uuid::new_v4(),
                            role: MessageRole::System,
                            content: format!("Error: {}", message),
                            blocks: vec![],
                            persona: None,
                        });
                    });
                    set_is_streaming.set(false);
                }
                ChatServerMessage::Status { .. } | ChatServerMessage::Pong => {
                    // Status updates - could show in UI
                }
            }
        }
    });

    // Handle message submission
    let on_submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        let text = input.get();
        if text.trim().is_empty() {
            return;
        }

        // Check WebSocket is connected
        if ws_state.get() != WsState::Connected {
            gloo_console::error!("WebSocket not connected");
            return;
        }

        // Add user message
        set_messages.update(|msgs| {
            msgs.push(ChatMessage {
                id: Uuid::new_v4(),
                role: MessageRole::User,
                content: text.clone(),
                blocks: vec![],
                persona: None,
            });
        });

        // Send to AI via WebSocket
        send_message(ChatClientMessage::Chat {
            text: text.clone(),
            persona: None,
        });

        // Clear input and show streaming state
        set_input.set(String::new());
        set_is_streaming.set(true);
    };

    // Connection status indicator
    let connection_status = move || match ws_state.get() {
        WsState::Connected => ("Connected", "bg-green-500"),
        WsState::Connecting => ("Connecting...", "bg-yellow-500"),
        WsState::Disconnected => ("Disconnected", "bg-red-500"),
        WsState::Error => ("Error", "bg-red-700"),
    };

    view! {
        <div class="flex flex-col h-[calc(100vh-8rem)]">
            // Chat header
            <div class="flex items-center justify-between pb-4 border-b border-gray-700">
                <h1 class="text-2xl font-bold">"Chat"</h1>
                <div class="flex items-center space-x-4">
                    // Connection status
                    <div class="flex items-center space-x-2">
                        <div class={move || format!("w-2 h-2 rounded-full {}", connection_status().1)} />
                        <span class="text-sm text-gray-400">{move || connection_status().0}</span>
                    </div>
                    <span class="text-sm text-gray-400">
                        "Session: " {move || session_id().to_string()[..8].to_string()}
                    </span>
                    <button
                        class="px-3 py-1 text-sm bg-gray-700 rounded hover:bg-gray-600"
                        on:click=move |_| {
                            set_messages.set(Vec::new());
                        }
                    >
                        "New Chat"
                    </button>
                </div>
            </div>

            // Messages area
            <div class="flex-1 overflow-y-auto py-4 space-y-4">
                <For
                    each=move || messages.get()
                    key=|msg| msg.id
                    let:message
                >
                    <MessageBubble
                        role=message.role
                        content=message.content.clone()
                        blocks=message.blocks.clone()
                    />
                </For>

                // Streaming indicator
                <Show when=move || is_streaming.get()>
                    <div class="flex items-center space-x-2 text-gray-400">
                        <div class="flex space-x-1">
                            <div class="w-2 h-2 bg-blue-500 rounded-full animate-bounce" style="animation-delay: 0ms" />
                            <div class="w-2 h-2 bg-blue-500 rounded-full animate-bounce" style="animation-delay: 150ms" />
                            <div class="w-2 h-2 bg-blue-500 rounded-full animate-bounce" style="animation-delay: 300ms" />
                        </div>
                        <span class="text-sm">"AI is thinking..."</span>
                    </div>
                </Show>
            </div>

            // Input area
            <form on:submit=on_submit class="pt-4 border-t border-gray-700">
                <div class="flex space-x-4">
                    <input
                        type="text"
                        placeholder="Type your message..."
                        class="flex-1 px-4 py-3 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500"
                        prop:value=move || input.get()
                        on:input=move |ev| set_input.set(event_target_value(&ev))
                        disabled=move || ws_state.get() != WsState::Connected
                    />
                    <button
                        type="submit"
                        class="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
                        disabled=move || {
                            input.get().trim().is_empty()
                                || is_streaming.get()
                                || ws_state.get() != WsState::Connected
                        }
                    >
                        "Send"
                    </button>
                </div>
            </form>
        </div>
    }
}

/// Chat message structure
#[derive(Clone, Debug)]
struct ChatMessage {
    id: Uuid,
    role: MessageRole,
    content: String,
    blocks: Vec<ContentBlock>,
    persona: Option<String>,
}

/// Message role
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Content block types
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ContentBlock {
    Markdown(String),
    Code { language: String, content: String },
    Diagram { diagram_type: String, source: String },
    Image { url: String, alt: String },
}
