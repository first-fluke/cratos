# Native Apps - Tauri 기반 데스크톱 앱

## 개요

Tauri를 사용하여 Cratos를 Windows, macOS, Linux 네이티브 데스크톱 앱으로 배포합니다.

### Tauri vs Electron 비교

| 항목 | Tauri | Electron |
|------|-------|----------|
| **바이너리 크기** | 3-10 MB | 150+ MB |
| **메모리 사용량** | 20-50 MB | 200+ MB |
| **백엔드 언어** | Rust | Node.js |
| **보안** | 높음 (Rust) | 보통 |
| **시스템 접근** | 네이티브 | Node.js API |

### 핵심 기능

| 기능 | 설명 |
|------|------|
| **시스템 트레이** | 백그라운드 실행, 빠른 접근 |
| **글로벌 단축키** | 어디서든 Cratos 호출 |
| **네이티브 알림** | OS 네이티브 알림 |
| **자동 업데이트** | Sparkle/WinSparkle 기반 |
| **클립보드 통합** | 복사/붙여넣기 자동 감지 |
| **파일 시스템 접근** | 로컬 파일 직접 조작 |

## 아키텍처

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

## 프로젝트 구조

```
cratos/
├── apps/
│   └── desktop/                 # Tauri 앱
│       ├── src/                 # React 프론트엔드
│       │   ├── App.tsx
│       │   ├── components/
│       │   └── hooks/
│       ├── src-tauri/           # Rust 백엔드
│       │   ├── src/
│       │   │   ├── main.rs
│       │   │   ├── commands.rs  # IPC 커맨드
│       │   │   ├── tray.rs      # 시스템 트레이
│       │   │   └── shortcuts.rs # 글로벌 단축키
│       │   ├── Cargo.toml
│       │   └── tauri.conf.json
│       ├── package.json
│       └── vite.config.ts
└── crates/
    └── cratos-core/             # 공유 코어 로직
```

## 의존성

### Rust (src-tauri/Cargo.toml)

```toml
[package]
name = "cratos-desktop"
version = "0.1.0"
edition = "2021"

[dependencies]
# Tauri 코어
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

# Cratos 코어
cratos-core = { path = "../../crates/cratos-core" }
cratos-llm = { path = "../../crates/cratos-llm" }

# 일반
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

## Tauri 설정 (tauri.conf.json)

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

## IPC 커맨드

### Rust 측 (commands.rs)

```rust
use cratos_core::{Orchestrator, OrchestratorInput};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;

/// 앱 상태
pub struct AppState {
    pub orchestrator: Arc<Mutex<Orchestrator>>,
}

/// 메시지 처리
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

/// 스트리밍 메시지 처리
#[tauri::command]
pub async fn process_message_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    message: String,
) -> Result<(), String> {
    let orchestrator = state.orchestrator.lock().await;

    // 스트리밍 콜백
    let callback = move |chunk: &str| {
        let _ = app.emit("llm-chunk", chunk);
    };

    orchestrator.process_streaming(&message, callback).await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 설정 조회
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let orchestrator = state.orchestrator.lock().await;
    let settings = orchestrator.get_settings();
    serde_json::to_value(settings).map_err(|e| e.to_string())
}

/// 설정 저장
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: serde_json::Value,
) -> Result<(), String> {
    let mut orchestrator = state.orchestrator.lock().await;
    orchestrator.update_settings(settings).map_err(|e| e.to_string())
}
```

### Frontend 측 (api.ts)

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// 일반 메시지 처리
export async function processMessage(message: string): Promise<string> {
  return invoke('process_message', { message });
}

// 스트리밍 메시지 처리
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

// 설정 API
export async function getSettings(): Promise<Settings> {
  return invoke('get_settings');
}

export async function saveSettings(settings: Settings): Promise<void> {
  return invoke('save_settings', { settings });
}
```

## 시스템 트레이

### Rust (tray.rs)

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Cratos 열기", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "설정", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;

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

## 글로벌 단축키

### Rust (shortcuts.rs)

```rust
use tauri::{AppHandle, GlobalShortcutManager, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn setup_shortcuts(app: &AppHandle) -> tauri::Result<()> {
    // Cmd/Ctrl + Shift + C: Cratos 토글
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

    // Cmd/Ctrl + Shift + V: 클립보드 내용으로 질문
    let clipboard_shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV);

    app.global_shortcut().on_shortcut(clipboard_shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            // 클립보드 내용 가져와서 처리
            let _ = app.emit("clipboard-query", ());
        }
    })?;

    Ok(())
}
```

## 네이티브 알림

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

// 예시: AI 응답 완료 알림
export async function notifyAIComplete(summary: string): Promise<void> {
  await notify('Cratos', `AI 응답 완료: ${summary.slice(0, 50)}...`);
}
```

## 자동 업데이트

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

## 빌드 및 배포

### 개발 모드

```bash
cd apps/desktop

# 의존성 설치
npm install

# 개발 서버 실행
npm run tauri dev
```

### 프로덕션 빌드

```bash
# 모든 플랫폼
npm run tauri build

# 특정 플랫폼
npm run tauri build -- --target x86_64-apple-darwin     # macOS Intel
npm run tauri build -- --target aarch64-apple-darwin   # macOS Apple Silicon
npm run tauri build -- --target x86_64-pc-windows-msvc # Windows
npm run tauri build -- --target x86_64-unknown-linux-gnu # Linux
```

### 결과물

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

## 설정

```toml
# config/default.toml
[desktop]
# 시스템 트레이
start_minimized = false
minimize_to_tray = true
show_tray_icon = true

# 글로벌 단축키
[desktop.shortcuts]
toggle = "CommandOrControl+Shift+C"
clipboard_query = "CommandOrControl+Shift+V"
quick_capture = "CommandOrControl+Shift+N"

# 자동 시작
[desktop.autostart]
enabled = false
minimized = true

# 업데이트
[desktop.updater]
enabled = true
check_on_startup = true
check_interval_hours = 24
```

## 보안 고려사항

1. **CSP 설정**: XSS 방지를 위한 Content Security Policy
2. **IPC 검증**: 모든 Tauri 커맨드에 입력 검증
3. **권한 최소화**: 필요한 Tauri 기능만 활성화
4. **서명**: 코드 서명으로 무결성 보장 (macOS notarization, Windows signing)

## 향후 계획

1. **v1.0**: 기본 데스크톱 앱 (채팅, 설정)
2. **v1.1**: 시스템 트레이 + 글로벌 단축키
3. **v1.2**: 자동 업데이트
4. **v2.0**: Live Canvas 통합
