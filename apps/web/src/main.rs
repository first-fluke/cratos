use cratos_web::App;
use leptos::*;

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        // Initialize tracing for WASM
        tracing_wasm::set_as_global_default();
        
        tracing::info!("Starting Cratos Web Dashboard");

        // Mount the app
        mount_to_body(|| view! { <App /> });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        eprintln!("❌ This binary is intended for the browser (WASM).");
        eprintln!("   Please use `trunk serve` to run the development server.");
    }
}
