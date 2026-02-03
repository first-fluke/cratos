//! Message Bubble Component

use leptos::*;

use crate::components::{CodeBlock, MarkdownBlock};
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
                                    <DiagramBlock diagram_type=diagram_type source=source />
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

/// Diagram block component (renders via Kroki)
#[component]
fn DiagramBlock(
    #[prop(into)] diagram_type: String,
    #[prop(into)] source: String,
) -> impl IntoView {
    let diagram_url = create_kroki_url(&diagram_type, &source);

    view! {
        <div class="bg-gray-900 rounded-lg p-4">
            <div class="text-xs text-gray-400 mb-2">
                {diagram_type.clone()} " diagram"
            </div>
            <img
                src=diagram_url
                alt="Diagram"
                class="max-w-full"
                loading="lazy"
            />
        </div>
    }
}

/// Create a Kroki URL for diagram rendering
fn create_kroki_url(diagram_type: &str, source: &str) -> String {
    use base64::Engine;
    use std::io::Write;

    let kroki_type = match diagram_type {
        "mermaid" => "mermaid",
        "plantuml" => "plantuml",
        "graphviz" | "dot" => "graphviz",
        "d2" => "d2",
        _ => "mermaid",
    };

    // Compress and encode
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = encoder.write_all(source.as_bytes());
    let compressed = encoder.finish().unwrap_or_default();
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed);

    format!("https://kroki.io/{}/svg/{}", kroki_type, encoded)
}
