//! Cratos Web Dashboard Entry Point

use cratos_web::App;
use leptos::*;

fn main() {
    // Initialize tracing for WASM
    tracing_wasm::set_as_global_default();

    tracing::info!("Starting Cratos Web Dashboard");

    // Mount the app
    mount_to_body(|| view! { <App /> });
}
