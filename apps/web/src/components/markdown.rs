//! Markdown Rendering Component

use leptos::*;
use pulldown_cmark::{html, Options, Parser};

/// Markdown block component
#[component]
pub fn MarkdownBlock(#[prop(into)] content: String) -> impl IntoView {
    let html = render_markdown(&content);

    view! {
        <div
            class="prose dark:prose-invert prose-sm max-w-none text-theme-primary"
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
        .replace("<h1>", r#"<h1 class="text-2xl font-bold mt-6 mb-4 text-theme-primary">"#)
        .replace("<h2>", r#"<h2 class="text-xl font-semibold mt-5 mb-3 text-theme-primary">"#)
        .replace("<h3>", r#"<h3 class="text-lg font-medium mt-4 mb-2 text-theme-primary">"#)
        .replace("<p>", r#"<p class="my-3 text-theme-secondary">"#)
        .replace("<ul>", r#"<ul class="list-disc list-inside my-3 space-y-1 text-theme-secondary">"#)
        .replace("<ol>", r#"<ol class="list-decimal list-inside my-3 space-y-1 text-theme-secondary">"#)
        .replace("<li>", r#"<li class="ml-4">"#)
        .replace("<blockquote>", r#"<blockquote class="border-l-4 border-gray-300 dark:border-gray-600 pl-4 my-3 italic text-gray-600 dark:text-gray-400">"#)
        .replace("<code>", r#"<code class="px-1 py-0.5 bg-gray-100 dark:bg-gray-800 rounded text-sm font-mono text-gray-800 dark:text-gray-200">"#)
        .replace("<pre>", r#"<pre class="p-4 bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg overflow-x-auto my-3">"#)
        .replace("<table>", r#"<table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700 my-3">"#)
        .replace("<thead>", r#"<thead class="bg-gray-50 dark:bg-gray-800">"#)
        .replace("<th>", r#"<th class="px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">"#)
        .replace("<td>", r#"<td class="px-4 py-2 text-sm text-theme-secondary">"#)
        .replace("<a ", r#"<a class="text-blue-600 dark:text-blue-400 hover:underline" "#)
        .replace("<strong>", r#"<strong class="font-semibold text-theme-primary">"#)
        .replace("<em>", r#"<em class="italic text-theme-secondary">"#)
        .replace("<hr>", r#"<hr class="my-6 border-gray-200 dark:border-gray-700">"#);

    html_output
}
