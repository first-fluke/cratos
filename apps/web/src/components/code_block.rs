//! Code Block Component
//!
//! Renders code with syntax highlighting.
//! In WASM, we use a simple HTML-based highlighting approach.

use leptos::*;

/// Code block with syntax highlighting
#[component]
pub fn CodeBlock(
    #[prop(into)] code: String,
    #[prop(into)] language: String,
    #[prop(optional)] executable: bool,
) -> impl IntoView {
    let code_clone = code.clone();
    let (copied, set_copied) = create_signal(false);

    // Copy to clipboard
    let on_copy = move |_| {
        let code_to_copy = code_clone.clone();
        spawn_local(async move {
            if let Some(window) = web_sys::window() {
                let clipboard = window.navigator().clipboard();
                let _ = wasm_bindgen_futures::JsFuture::from(
                    clipboard.write_text(&code_to_copy)
                ).await;
                set_copied.set(true);
                gloo_timers::future::TimeoutFuture::new(2000).await;
                set_copied.set(false);
            }
        });
    };

    // Simple keyword highlighting for common languages
    let highlighted_code = highlight_code(&code, &language);

    view! {
        <div class="relative group">
            // Language badge
            <div class="absolute top-0 left-0 px-2 py-1 text-xs font-mono text-gray-400 bg-gray-800 rounded-tl-lg">
                {language.clone()}
            </div>

            // Copy button
            <button
                class="absolute top-2 right-2 px-2 py-1 text-xs bg-gray-700 rounded opacity-0 group-hover:opacity-100 transition-opacity"
                on:click=on_copy
            >
                {move || if copied.get() { "Copied!" } else { "Copy" }}
            </button>

            // Execute button (if executable)
            <Show when=move || executable>
                <button class="absolute top-2 right-16 px-2 py-1 text-xs bg-green-700 rounded opacity-0 group-hover:opacity-100 transition-opacity">
                    "Run"
                </button>
            </Show>

            // Code content
            <pre class="p-4 pt-8 bg-gray-900 rounded-lg overflow-x-auto">
                <code class="text-sm font-mono" inner_html=highlighted_code />
            </pre>
        </div>
    }
}

/// Simple syntax highlighting
fn highlight_code(code: &str, language: &str) -> String {
    let keywords = get_keywords(language);
    let mut result = html_escape(code);

    // Highlight keywords
    for keyword in keywords {
        let pattern = format!(r"\b{}\b", keyword);
        let replacement = format!(r#"<span class="text-purple-400">{}</span>"#, keyword);
        result = result.replace(&pattern, &replacement);
    }

    // Highlight strings
    result = highlight_strings(&result);

    // Highlight comments
    result = highlight_comments(&result, language);

    // Highlight numbers
    result = highlight_numbers(&result);

    result
}

/// Get keywords for a language
fn get_keywords(language: &str) -> Vec<&'static str> {
    match language {
        "rust" => vec![
            "fn", "let", "mut", "const", "static", "pub", "use", "mod", "struct", "enum",
            "impl", "trait", "for", "while", "loop", "if", "else", "match", "return",
            "async", "await", "self", "Self", "true", "false", "where", "type", "dyn",
        ],
        "python" => vec![
            "def", "class", "if", "elif", "else", "for", "while", "try", "except", "finally",
            "import", "from", "as", "return", "yield", "with", "async", "await", "True", "False",
            "None", "and", "or", "not", "in", "is", "lambda", "pass", "raise", "break", "continue",
        ],
        "javascript" | "typescript" => vec![
            "function", "const", "let", "var", "if", "else", "for", "while", "do", "switch",
            "case", "break", "continue", "return", "try", "catch", "finally", "throw", "class",
            "extends", "import", "export", "from", "async", "await", "true", "false", "null",
            "undefined", "new", "this", "typeof", "instanceof",
        ],
        "go" => vec![
            "func", "var", "const", "type", "struct", "interface", "if", "else", "for", "range",
            "switch", "case", "default", "return", "break", "continue", "go", "chan", "select",
            "defer", "package", "import", "true", "false", "nil", "make", "new", "map",
        ],
        _ => vec![],
    }
}

/// Escape HTML special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Highlight string literals
fn highlight_strings(code: &str) -> String {
    // This is a simplified version - a proper implementation would use regex
    let mut result = String::new();
    let mut in_string = false;
    let mut string_char = ' ';
    let chars = code.chars().peekable();

    for c in chars {
        if !in_string && (c == '"' || c == '\'') {
            in_string = true;
            string_char = c;
            result.push_str(r#"<span class="text-green-400">"#);
            result.push(c);
        } else if in_string && c == string_char && !result.ends_with('\\') {
            result.push(c);
            result.push_str("</span>");
            in_string = false;
        } else {
            result.push(c);
        }
    }

    result
}

/// Highlight comments (simplified)
fn highlight_comments(code: &str, language: &str) -> String {
    let comment_prefix = match language {
        "python" => "#",
        _ => "//",
    };

    code.lines()
        .map(|line| {
            if let Some(idx) = line.find(comment_prefix) {
                let (code_part, comment_part) = line.split_at(idx);
                format!(
                    "{}<span class=\"text-gray-500\">{}</span>",
                    code_part, comment_part
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Highlight numbers (simplified)
fn highlight_numbers(code: &str) -> String {
    let mut result = String::new();
    let mut chars = code.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            result.push_str(r#"<span class="text-yellow-400">"#);
            result.push(c);
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit() || next == '.' || next == '_' {
                    result.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            result.push_str("</span>");
        } else {
            result.push(c);
        }
    }

    result
}
