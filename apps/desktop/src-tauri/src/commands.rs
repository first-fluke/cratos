use tauri::{State, Wry};
use crate::state::AppState; 

// Concrete type for commands
type ConcreteAppState = AppState<Wry>;

#[tauri::command]
pub async fn connect_server(
    state: State<'_, ConcreteAppState>,
    url: String,
) -> Result<(), String> {
    state.ws_manager.connect(&url).await
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, ConcreteAppState>,
    message: String,
) -> Result<(), String> {
    state.ws_manager.send_chat(message).await
}

#[tauri::command]
pub async fn start_voice(
    state: State<'_, ConcreteAppState>,
    _mode: String,
) -> Result<(), String> {
    state.voice_controller.start_capture().await
}

#[tauri::command]
pub async fn stop_voice(state: State<'_, ConcreteAppState>) -> Result<(), String> {
    state.voice_controller.stop_capture().await.map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct AppStatus {
    connected: bool,
}

#[tauri::command]
pub async fn get_status(state: State<'_, ConcreteAppState>) -> Result<AppStatus, String> {
    Ok(AppStatus {
        connected: state.ws_manager.is_connected().await,
    })
}
