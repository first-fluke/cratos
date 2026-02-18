#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;
mod state;
mod tray;
mod websocket;
mod voice;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Setup global state
            let app_handle = app.handle().clone();
            // Note: AppState is now created slightly differently if needed, 
            // but `state::AppState::new` already takes app_handle.
            // However, `new` now creates internal Arc for voice controller.
            let state = state::AppState::new(app_handle.clone());
            app.manage(state);

            // Create tray
            #[cfg(target_os = "macos")]
            tray::create_tray(&app_handle)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_server,
            commands::send_message,
            commands::get_status,
            commands::start_voice,
            commands::stop_voice,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
