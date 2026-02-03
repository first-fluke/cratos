//! Chat Page

use leptos::*;
use leptos_router::*;
use uuid::Uuid;

use crate::api::websocket::use_canvas_websocket;
use crate::components::MessageBubble;

/// Chat page with canvas support
#[component]
pub fn Chat() -> impl IntoView {
    let params = use_params_map();

    // Get or create session ID
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

    // WebSocket connection
    let ws = use_canvas_websocket(session_id);
    let (_, _, send_message) = ws;

    // Handle message submission
    let on_submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        let text = input.get();
        if text.trim().is_empty() {
            return;
        }

        // Add user message
        set_messages.update(|msgs| {
            msgs.push(ChatMessage {
                id: Uuid::new_v4(),
                role: MessageRole::User,
                content: text.clone(),
                blocks: Vec::new(),
            });
        });

        // Send to AI via WebSocket
        send_message(serde_json::json!({
            "type": "ask_ai",
            "prompt": text,
            "context_blocks": []
        }));

        // Clear input and show streaming state
        set_input.set(String::new());
        set_is_streaming.set(true);

        // Simulate AI response (in production, handled by WebSocket messages)
        spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(1000).await;
            set_messages.update(|msgs| {
                msgs.push(ChatMessage {
                    id: Uuid::new_v4(),
                    role: MessageRole::Assistant,
                    content: format!("AI response to: {}", text),
                    blocks: vec![
                        ContentBlock::Markdown("Here's a sample response with **bold** and *italic* text.".to_string()),
                        ContentBlock::Code {
                            language: "rust".to_string(),
                            content: "fn main() {\n    println!(\"Hello, world!\");\n}".to_string(),
                        },
                    ],
                });
            });
            set_is_streaming.set(false);
        });
    };

    view! {
        <div class="flex flex-col h-[calc(100vh-8rem)]">
            // Chat header
            <div class="flex items-center justify-between pb-4 border-b border-gray-700">
                <h1 class="text-2xl font-bold">"Chat"</h1>
                <div class="flex items-center space-x-4">
                    <span class="text-sm text-gray-400">
                        "Session: " {move || session_id().to_string()[..8].to_string()}
                    </span>
                    <button class="px-3 py-1 text-sm bg-gray-700 rounded hover:bg-gray-600">
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
                    />
                    <button
                        type="submit"
                        class="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
                        disabled=move || input.get().trim().is_empty() || is_streaming.get()
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
