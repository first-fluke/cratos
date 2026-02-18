use futures_util::stream::{SplitSink, SplitStream, StreamExt};
use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tauri::{Emitter, Runtime}; 
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock}; 
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use url::Url;

// Type alias because tokio-tungstenite streams are complex
type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSender = SplitSink<WsStream, Message>;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatMessage {
    pub content: String,
}

pub struct WebSocketManager<R: Runtime> {
    app_handle: tauri::AppHandle<R>,
    sender: Arc<Mutex<Option<WsSender>>>,
    connected: Arc<RwLock<bool>>,
}

impl<R: Runtime> WebSocketManager<R> {
    pub fn new(app_handle: tauri::AppHandle<R>) -> Self {
        Self {
            app_handle,
            sender: Arc::new(Mutex::new(None)),
            connected: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn connect(&self, url_str: &str) -> Result<(), String> {
        let url = Url::parse(url_str).map_err(|e| e.to_string())?;

        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                let (write, read) = ws_stream.split();
                
                // Store writer
                let mut sender_guard = self.sender.lock().await;
                *sender_guard = Some(write);
                
                // Update status
                let mut connected_guard = self.connected.write().await;
                *connected_guard = true;
                drop(connected_guard); // release lock

                // Spawn reader task
                let app_handle = self.app_handle.clone();
                let connected_flag = self.connected.clone();
                
                tokio::spawn(async move {
                    Self::handle_messages(read, app_handle, connected_flag).await;
                });

                Ok(())
            }
            Err(e) => Err(format!("Failed to connect: {}", e)),
        }
    }

    pub async fn send_chat(&self, message: String) -> Result<(), String> {
        self.send(Message::Text(json!({
            "type": "chat",
            "content": message
        }).to_string())).await
    }

    pub async fn send_binary(&self, data: Vec<u8>) -> Result<(), String> {
        self.send(Message::Binary(data)).await
    }

    async fn send(&self, msg: Message) -> Result<(), String> {
        let mut sender_guard = self.sender.lock().await;
        if let Some(sender) = sender_guard.as_mut() {
            sender.send(msg).await.map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Not connected".to_string())
        }
    }

    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    async fn handle_messages(
        mut read: SplitStream<WsStream>,
        app_handle: tauri::AppHandle<R>,
        connected_flag: Arc<RwLock<bool>>,
    ) {
        while let Some(msg_result) = read.next().await {
            match msg_result {
                Ok(msg) => {
                    match msg {
                        Message::Text(text) => {
                            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text) {
                                let _ = app_handle.emit("a2ui-message", json_val);
                            } else {
                                let _ = app_handle.emit("server-message-raw", text); 
                            }
                        }
                        Message::Binary(_bin) => { // Fixed unused variable
                            // Handle audio data from server if needed (for TTS playback)
                            // For now, emit event for frontend or handle in Rust
                        }
                        Message::Close(_) => {
                            break;
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    break;
                }
            }
        }

        // Connection closed
        let mut connected_guard = connected_flag.write().await;
        *connected_guard = false;
        let _ = app_handle.emit("connection-status", json!({"connected": false}));
    }
}
