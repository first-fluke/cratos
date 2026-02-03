//! WebSocket Client for Canvas

use leptos::*;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

/// WebSocket connection state
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WsState {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

/// Canvas WebSocket hook
pub fn use_canvas_websocket<F>(
    session_id: F,
) -> (
    ReadSignal<WsState>,
    ReadSignal<Option<serde_json::Value>>,
    impl Fn(serde_json::Value) + Clone,
)
where
    F: Fn() -> Uuid + 'static,
{
    let (state, set_state) = create_signal(WsState::Disconnected);
    let (last_message, set_last_message) = create_signal::<Option<serde_json::Value>>(None);
    let (ws, set_ws) = create_signal::<Option<WebSocket>>(None);

    // Connect to WebSocket
    create_effect(move |_| {
        let session = session_id();
        let url = get_websocket_url(&session);

        set_state.set(WsState::Connecting);

        match WebSocket::new(&url) {
            Ok(socket) => {
                // Set binary type
                socket.set_binary_type(web_sys::BinaryType::Arraybuffer);

                // onopen handler
                let set_state_open = set_state;
                let onopen = Closure::wrap(Box::new(move |_: JsValue| {
                    gloo_console::log!("WebSocket connected");
                    set_state_open.set(WsState::Connected);
                }) as Box<dyn Fn(JsValue)>);
                socket.set_onopen(Some(onopen.as_ref().unchecked_ref()));
                onopen.forget();

                // onmessage handler
                let set_last_message_msg = set_last_message;
                let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
                    if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                        let text_string: String = text.into();
                        if let Ok(json) = serde_json::from_str(&text_string) {
                            set_last_message_msg.set(Some(json));
                        }
                    }
                }) as Box<dyn Fn(MessageEvent)>);
                socket.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
                onmessage.forget();

                // onerror handler
                let set_state_error = set_state;
                let onerror = Closure::wrap(Box::new(move |e: ErrorEvent| {
                    gloo_console::error!("WebSocket error:", e.message());
                    set_state_error.set(WsState::Error);
                }) as Box<dyn Fn(ErrorEvent)>);
                socket.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                onerror.forget();

                // onclose handler
                let set_state_close = set_state;
                let onclose = Closure::wrap(Box::new(move |e: CloseEvent| {
                    gloo_console::log!("WebSocket closed:", e.code(), e.reason());
                    set_state_close.set(WsState::Disconnected);
                }) as Box<dyn Fn(CloseEvent)>);
                socket.set_onclose(Some(onclose.as_ref().unchecked_ref()));
                onclose.forget();

                set_ws.set(Some(socket));
            }
            Err(e) => {
                gloo_console::error!("Failed to create WebSocket:", format!("{:?}", e));
                set_state.set(WsState::Error);
            }
        }
    });

    // Send message function
    let send_message = move |msg: serde_json::Value| {
        if let Some(socket) = ws.get() {
            if let Ok(text) = serde_json::to_string(&msg) {
                if let Err(e) = socket.send_with_str(&text) {
                    gloo_console::error!("Failed to send message:", format!("{:?}", e));
                }
            }
        }
    };

    // Cleanup on unmount
    on_cleanup(move || {
        if let Some(socket) = ws.get() {
            let _ = socket.close();
        }
    });

    (state, last_message, send_message)
}

/// Get WebSocket URL for a session
fn get_websocket_url(session_id: &Uuid) -> String {
    let window = web_sys::window().expect("no global window");
    let location = window.location();

    let protocol = if location.protocol().unwrap_or_default() == "https:" {
        "wss:"
    } else {
        "ws:"
    };

    let host = location.host().unwrap_or_else(|_| "localhost:8080".to_string());

    format!(
        "{}//{}/api/v1/canvas/ws/{}",
        protocol, host, session_id
    )
}
