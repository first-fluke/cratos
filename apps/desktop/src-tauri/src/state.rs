use crate::websocket::WebSocketManager;
use crate::voice::VoiceController;
use std::sync::Arc;
use tauri::Runtime;

pub struct AppState<R: Runtime> {
    pub ws_manager: Arc<WebSocketManager<R>>,
    pub voice_controller: Arc<VoiceController<R>>,
}

impl<R: Runtime> AppState<R> {
    pub fn new(app_handle: tauri::AppHandle<R>) -> Self {
        let ws_manager = Arc::new(WebSocketManager::new(app_handle.clone()));
        let voice_controller = Arc::new(VoiceController::new(app_handle, ws_manager.clone()));
        
        Self {
            ws_manager,
            voice_controller,
        }
    }
}
