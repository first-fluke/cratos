# Native Apps - Tauri-based Desktop Application

## Overview

Deploy Cratos as a native desktop app for Windows, macOS, and Linux using Tauri.

### Tauri vs Electron Comparison

| Item | Tauri | Electron |
|------|-------|----------|
| **Binary Size** | 3-10 MB | 150+ MB |
| **Memory Usage** | 20-50 MB | 200+ MB |
| **Backend Language** | Rust | Node.js |
| **Security** | High (Rust) | Moderate |
| **System Access** | Native | Node.js API |

### Core Features

| Feature | Description |
|---------|-------------|
| **System Tray** | Background execution, quick access |
| **Global Shortcuts** | Invoke Cratos from anywhere |
| **Native Notifications** | OS native notifications |
| **Auto Update** | Sparkle/WinSparkle based |
| **Clipboard Integration** | Auto-detect copy/paste |
| **File System Access** | Direct local file manipulation |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Tauri Application                         │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                   WebView (Frontend)                     │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │ │
│  │  │   React     │  │   Canvas    │  │    Settings     │  │ │
│  │  │   Chat UI   │  │   (Monaco)  │  │    Panel        │  │ │
│  │  └─────────────┘  └─────────────┘  └─────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
│                           │                                   │
│                    Tauri IPC Bridge                           │
│                           │                                   │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                   Rust Backend                           │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │ │
│  │  │   Cratos    │  │   System    │  │     Update      │  │ │
│  │  │   Core      │  │   APIs      │  │     Manager     │  │ │
│  │  └─────────────┘  └─────────────┘  └─────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────────┐
│                    Operating System                          │
│  [Tray Icon] [Global Shortcuts] [Notifications] [Clipboard] │
└─────────────────────────────────────────────────────────────┘
```

## Project Structure

```
cratos/
├── apps/
│   └── desktop/                 # Tauri app
│       ├── src/                 # React frontend
│       │   ├── App.tsx
│       │   ├── components/
│       │   └── hooks/
│       ├── src-tauri/           # Rust backend
│       │   ├── src/
│       │   │   ├── main.rs
│       │   │   ├── commands.rs  # IPC commands
│       │   │   ├── tray.rs      # System tray
│       │   │   └── shortcuts.rs # Global shortcuts
│       │   ├── Cargo.toml
│       │   └── tauri.conf.json
│       ├── package.json
│       └── vite.config.ts
└── crates/
    └── cratos-core/             # Shared core logic
```

## Dependencies

### Rust (src-tauri/Cargo.toml)

```toml
[package]
name = "cratos-desktop"
version = "0.1.0"
edition = "2021"

[dependencies]
# Tauri core
tauri = { version = "2.0", features = [
    "tray-icon",
    "global-shortcut",
    "notification",
    "clipboard-manager",
    "dialog",
    "fs",
    "shell",
    "updater",
] }
tauri-plugin-autostart = "2.0"
tauri-plugin-single-instance = "2.0"
tauri-plugin-store = "2.0"

# Cratos core
cratos-core = { path = "../../crates/cratos-core" }
cratos-llm = { path = "../../crates/cratos-llm" }

# General
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
```

### Frontend (package.json)

```json
{
  "name": "cratos-desktop",
  "version": "0.1.0",
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "@tauri-apps/plugin-autostart": "^2.0.0",
    "@tauri-apps/plugin-global-shortcut": "^2.0.0",
    "@tauri-apps/plugin-notification": "^2.0.0",
    "@tauri-apps/plugin-clipboard-manager": "^2.0.0",
    "react": "^18.2.0",
    "react-dom": "^18.2.0",
    "@monaco-editor/react": "^4.6.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "vite": "^5.0.0",
    "@vitejs/plugin-react": "^4.0.0",
    "typescript": "^5.3.0"
  }
}
```

## Tauri Configuration (tauri.conf.json)

```json
{
  "productName": "Cratos",
  "version": "0.1.0",
  "identifier": "com.cratos.app",
  "build": {
    "beforeBuildCommand": "npm run build",
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
  },
  "app": {
    "withGlobalTauri": true,
    "trayIcon": {
      "iconPath": "icons/tray.png",
      "iconAsTemplate": true
    },
    "windows": [
      {
        "title": "Cratos",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "center": true,
        "decorations": true,
        "transparent": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["dmg", "nsis", "appimage"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "macOS": {
      "minimumSystemVersion": "10.15",
      "entitlements": "entitlements.plist",
      "signingIdentity": null
    },
    "windows": {
      "certificateThumbprint": null,
      "digestAlgorithm": "sha256"
    }
  },
  "plugins": {
    "updater": {
      "pubkey": "YOUR_PUBLIC_KEY",
      "endpoints": [
        "https://releases.cratos.dev/{{target}}/{{arch}}/{{current_version}}"
      ]
    }
  }
}
```

## IPC Commands

### Rust Side (commands.rs)

```rust
use cratos_core::{Orchestrator, OrchestratorInput};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;

/// App state
pub struct AppState {
    pub orchestrator: Arc<Mutex<Orchestrator>>,
}

/// Process message
#[tauri::command]
pub async fn process_message(
    state: State<'_, AppState>,
    message: String,
) -> Result<String, String> {
    let orchestrator = state.orchestrator.lock().await;

    let input = OrchestratorInput::new(
        "desktop",
        "local",
        "desktop_user",
        &message,
    );

    let result = orchestrator.process(input).await
        .map_err(|e| e.to_string())?;

    Ok(result.response)
}

/// Process streaming message
#[tauri::command]
pub async fn process_message_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    message: String,
) -> Result<(), String> {
    let orchestrator = state.orchestrator.lock().await;

    // Streaming callback
    let callback = move |chunk: &str| {
        let _ = app.emit("llm-chunk", chunk);
    };

    orchestrator.process_streaming(&message, callback).await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get settings
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let orchestrator = state.orchestrator.lock().await;
    let settings = orchestrator.get_settings();
    serde_json::to_value(settings).map_err(|e| e.to_string())
}

/// Save settings
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: serde_json::Value,
) -> Result<(), String> {
    let mut orchestrator = state.orchestrator.lock().await;
    orchestrator.update_settings(settings).map_err(|e| e.to_string())
}
```

### Frontend Side (api.ts)

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Process regular message
export async function processMessage(message: string): Promise<string> {
  return invoke('process_message', { message });
}

// Process streaming message
export async function processMessageStream(
  message: string,
  onChunk: (chunk: string) => void
): Promise<void> {
  const unlisten = await listen<string>('llm-chunk', (event) => {
    onChunk(event.payload);
  });

  try {
    await invoke('process_message_stream', { message });
  } finally {
    unlisten();
  }
}

// Settings API
export async function getSettings(): Promise<Settings> {
  return invoke('get_settings');
}

export async function saveSettings(settings: Settings): Promise<void> {
  return invoke('save_settings', { settings });
}
```

## System Tray

### Rust (tray.rs)

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open Cratos", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &settings, &quit])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("Cratos AI Assistant")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "settings" => {
                    let _ = app.emit("open-settings", ());
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
```

## Global Shortcuts

### Rust (shortcuts.rs)

```rust
use tauri::{AppHandle, GlobalShortcutManager, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn setup_shortcuts(app: &AppHandle) -> tauri::Result<()> {
    // Cmd/Ctrl + Shift + C: Toggle Cratos
    let toggle_shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyC);

    app.global_shortcut().on_shortcut(toggle_shortcut, |app, shortcut, event| {
        if event.state == ShortcutState::Pressed {
            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
    })?;

    // Cmd/Ctrl + Shift + V: Query with clipboard content
    let clipboard_shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV);

    app.global_shortcut().on_shortcut(clipboard_shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            // Get clipboard content and process
            let _ = app.emit("clipboard-query", ());
        }
    })?;

    Ok(())
}
```

## Native Notifications

### Frontend (notifications.ts)

```typescript
import {
  isPermissionGranted,
  requestPermission,
  sendNotification
} from '@tauri-apps/plugin-notification';

export async function notify(title: string, body: string): Promise<void> {
  let permissionGranted = await isPermissionGranted();

  if (!permissionGranted) {
    const permission = await requestPermission();
    permissionGranted = permission === 'granted';
  }

  if (permissionGranted) {
    sendNotification({
      title,
      body,
      icon: 'icons/notification.png',
    });
  }
}

// Example: AI response complete notification
export async function notifyAIComplete(summary: string): Promise<void> {
  await notify('Cratos', `AI response complete: ${summary.slice(0, 50)}...`);
}
```

## Auto Update

### Rust (updater.rs)

```rust
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

pub async fn check_for_updates(app: &AppHandle) -> tauri::Result<Option<Update>> {
    let updater = app.updater()?;

    match updater.check().await {
        Ok(Some(update)) => {
            tracing::info!(
                "Update available: {} -> {}",
                update.current_version,
                update.version
            );
            Ok(Some(update))
        }
        Ok(None) => {
            tracing::info!("No updates available");
            Ok(None)
        }
        Err(e) => {
            tracing::error!("Update check failed: {}", e);
            Err(e.into())
        }
    }
}

pub async fn install_update(update: Update) -> tauri::Result<()> {
    tracing::info!("Downloading update...");

    let mut downloaded = 0;
    let total = update.download_size.unwrap_or(0);

    update.download_and_install(
        |progress, _total| {
            downloaded += progress;
            tracing::debug!("Download progress: {}/{}", downloaded, total);
        },
        || {
            tracing::info!("Download complete, installing...");
        }
    ).await?;

    Ok(())
}
```

## Build and Deploy

### Development Mode

```bash
cd apps/desktop

# Install dependencies
npm install

# Run development server
npm run tauri dev
```

### Production Build

```bash
# All platforms
npm run tauri build

# Specific platform
npm run tauri build -- --target x86_64-apple-darwin     # macOS Intel
npm run tauri build -- --target aarch64-apple-darwin   # macOS Apple Silicon
npm run tauri build -- --target x86_64-pc-windows-msvc # Windows
npm run tauri build -- --target x86_64-unknown-linux-gnu # Linux
```

### Build Output

```
apps/desktop/src-tauri/target/release/bundle/
├── dmg/                    # macOS DMG
│   └── Cratos_0.1.0_x64.dmg
├── macos/                  # macOS App Bundle
│   └── Cratos.app
├── nsis/                   # Windows Installer
│   └── Cratos_0.1.0_x64-setup.exe
└── appimage/               # Linux AppImage
    └── Cratos_0.1.0_amd64.AppImage
```

## Configuration

```toml
# config/default.toml
[desktop]
# System tray
start_minimized = false
minimize_to_tray = true
show_tray_icon = true

# Global shortcuts
[desktop.shortcuts]
toggle = "CommandOrControl+Shift+C"
clipboard_query = "CommandOrControl+Shift+V"
quick_capture = "CommandOrControl+Shift+N"

# Auto start
[desktop.autostart]
enabled = false
minimized = true

# Update
[desktop.updater]
enabled = true
check_on_startup = true
check_interval_hours = 24
```

## Security Considerations

1. **CSP Settings**: Content Security Policy to prevent XSS
2. **IPC Validation**: Input validation for all Tauri commands
3. **Permission Minimization**: Enable only required Tauri features
4. **Code Signing**: Integrity assurance (macOS notarization, Windows signing)

## Roadmap

1. **v1.0**: Basic desktop app (chat, settings)
2. **v1.1**: System tray + global shortcuts
3. **v1.2**: Auto update
4. **v2.0**: Live Canvas integration
