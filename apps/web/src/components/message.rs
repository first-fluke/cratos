//! Message Bubble Component

use leptos::*;

use crate::components::{CodeBlock, Diagram, MarkdownBlock};
use crate::pages::chat::{ContentBlock, MessageRole};

/// Message bubble component
#[component]
pub fn MessageBubble(
    role: MessageRole,
    #[prop(into)] content: String,
    blocks: Vec<ContentBlock>,
) -> impl IntoView {
    let is_user = role == MessageRole::User;

    let bubble_class = if is_user {
        "bg-blue-900/50 ml-auto"
    } else {
        "bg-gray-800"
    };

    let align_class = if is_user { "justify-end" } else { "justify-start" };

    // Clone content for use in closures
    let content_for_check = content.clone();
    let content_for_render = content;

    view! {
        <div class={format!("flex {}", align_class)}>
            <div class={format!("max-w-[80%] rounded-lg p-4 {}", bubble_class)}>
                // Role indicator
                <div class="flex items-center space-x-2 mb-2">
                    <span class="text-xs font-medium text-gray-400">
                        {match role {
                            MessageRole::User => "You",
                            MessageRole::Assistant => "Cratos",
                            MessageRole::System => "System",
                        }}
                    </span>
                </div>

                // Main content
                <Show when=move || !content_for_check.is_empty()>
                    <MarkdownBlock content=content_for_render.clone() />
                </Show>

                // Content blocks
                <div class="space-y-4 mt-4">
                    {blocks.into_iter().map(|block| {
                        match block {
                            ContentBlock::Markdown(text) => {
                                view! {
                                    <MarkdownBlock content=text />
                                }.into_view()
                            }
                            ContentBlock::Code { language, content } => {
                                view! {
                                    <CodeBlock code=content language=language />
                                }.into_view()
                            }
                            ContentBlock::Diagram { diagram_type, source } => {
                                view! {
                                    <Diagram diagram_type=diagram_type source=source />
                                }.into_view()
                            }
                            ContentBlock::Image { url, alt } => {
                                view! {
                                    <img
                                        src=url
                                        alt=alt
                                        class="max-w-full rounded-lg"
                                        loading="lazy"
                                    />
                                }.into_view()
                            }
                        }
                    }).collect_view()}
                </div>
            </div>
        </div>
    }
}

