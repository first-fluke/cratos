//! Markdown Rendering Component

use leptos::*;
use pulldown_cmark::{html, Options, Parser};

/// Markdown block component
#[component]
pub fn MarkdownBlock(#[prop(into)] content: String) -> impl IntoView {
    let html = render_markdown(&content);

    view! {
        <div
            class="prose prose-invert prose-sm max-w-none"
            inner_html=html
        />
    }
}

/// Render markdown to HTML
fn render_markdown(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    // Apply Tailwind-compatible classes
    html_output = html_output
        .replace("<h1>", r#"<h1 class="text-2xl font-bold mt-6 mb-4">"#)
        .replace("<h2>", r#"<h2 class="text-xl font-semibold mt-5 mb-3">"#)
        .replace("<h3>", r#"<h3 class="text-lg font-medium mt-4 mb-2">"#)
        .replace("<p>", r#"<p class="my-3">"#)
        .replace("<ul>", r#"<ul class="list-disc list-inside my-3 space-y-1">"#)
        .replace("<ol>", r#"<ol class="list-decimal list-inside my-3 space-y-1">"#)
        .replace("<li>", r#"<li class="ml-4">"#)
        .replace("<blockquote>", r#"<blockquote class="border-l-4 border-gray-600 pl-4 my-3 italic text-gray-400">"#)
        .replace("<code>", r#"<code class="px-1 py-0.5 bg-gray-800 rounded text-sm font-mono">"#)
        .replace("<pre>", r#"<pre class="p-4 bg-gray-900 rounded-lg overflow-x-auto my-3">"#)
        .replace("<table>", r#"<table class="min-w-full divide-y divide-gray-700 my-3">"#)
        .replace("<thead>", r#"<thead class="bg-gray-800">"#)
        .replace("<th>", r#"<th class="px-4 py-2 text-left text-xs font-medium text-gray-400 uppercase">"#)
        .replace("<td>", r#"<td class="px-4 py-2 text-sm">"#)
        .replace("<a ", r#"<a class="text-blue-400 hover:underline" "#)
        .replace("<strong>", r#"<strong class="font-semibold">"#)
        .replace("<em>", r#"<em class="italic">"#)
        .replace("<hr>", r#"<hr class="my-6 border-gray-700">"#);

    html_output
}
