use cratos_web::App;
use leptos::*;

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        // Set panic hook for better error messages
        console_error_panic_hook::set_once();

        // Log to console
        web_sys::console::log_1(&"Starting Cratos Web Dashboard".into());

        // Mount the app
        mount_to_body(|| view! { <App /> });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        eprintln!("❌ This binary is intended for the browser (WASM).");
        eprintln!("   Please use `trunk serve` to run the development server.");
    }
}
