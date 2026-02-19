# Mobile Agent - Tech Stack Reference

## Core Stack
- **Framework**: Tauri Mobile (v2+)
- **Frontend**: Next.js / React (Shared with Web)
- **Styling**: Tailwind CSS (Touch utilities)

## Native Integration (Rust)
- **Plugins**: `tauri-plugin-*` (Official & Community)
- **Android**: Kotlin / Java (via JNI/Rust bridge)
- **iOS**: Swift / Objective-C (via Rust bridge)

## Development Tools
- **Android**: Android Studio, Gradle, ADB
- **iOS**: Xcode, CocoaPods (if needed)

## Capabilities (Permissions)
- Configured in `src-tauri/capabilities/*.json`
- Scoped to specific windows or platforms (`target: "android"`)

## UI Patterns
- **Safe Area**: `pt-[env(safe-area-inset-top)]`
- **Gestures**: `react-use-gesture` or native scroll
- **Haptics**: `tauri-plugin-haptics`
