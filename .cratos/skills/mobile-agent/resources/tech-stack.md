# Mobile Agent - Tech Stack Reference (Tauri Project)

## Core Stack

- **Framework**: Tauri Mobile (v2+)
- **Frontend**: Next.js / React (Shared with Web)
- **Language**: TypeScript (strict mode enabled)
- **Styling**: Tailwind CSS (mobile-safe utilities)
- **Build Tooling**: Vite / Next build pipeline

## Architecture Overview

- **UI Layer**: React components (presentation only)
- **State Layer**: Single strategy (Zustand / Redux Toolkit / Signals — no mixing)
- **IPC Layer**: `invoke()` boundary (typed request/response contracts)
- **Rust Layer**: Business logic + system access
- **Capability Layer**: Explicit permission configuration
- **Native Wrapper**: Android (WebView) / iOS (WKWebView)

Clear separation required:
Frontend (UI) → IPC → Rust → System / DB

---

## Native Integration (Rust)

- **Language**: Rust (stable toolchain)
- **Async Runtime**: Tokio
- **Serialization**: Serde (`Serialize` / `Deserialize`)
- **Error Handling**: Custom error type mapped to standardized frontend format
- **Plugins**: `tauri-plugin-*` (Official preferred)

Rules:
- No `unwrap()` in production
- Long-running tasks executed asynchronously
- Blocking I/O avoided on main thread

---

## IPC Contract Standard

- All commands defined with `#[tauri::command]`
- Request structs explicitly typed
- Response structs explicitly typed
- Standard error format:
  - `code`
  - `message`
  - `details` (optional)
- TypeScript types must mirror Rust structs exactly

---

## Capabilities (Permissions)

- Defined in `src-tauri/capabilities/*.json`
- Scoped by platform (`target: "android"` / `"ios"`)
- Strict capability mode enabled
- No unused permissions allowed
- Rebuild required after capability changes

---

## Security Strategy

- Input validation at Rust boundary
- File system access scoped
- Secrets never exposed to frontend bundle
- CSP configured for production
- Devtools disabled in release build
- iOS ATS compliance verified

---

## Performance Strategy

- Heavy computation moved to Rust
- No large synchronous loops in JS
- Images optimized for mobile memory
- Cold start measured (release build)
- Avoid excessive re-renders in React

---

## Logging & Observability

- Structured logging in Rust
- Frontend error logging (no sensitive data)
- Log levels separated: dev vs production
- Crash reproduction tested in release build

---

## Development Tools

- **Android**: Android Studio, Gradle, ADB
- **iOS**: Xcode
- **Rust**: cargo, clippy, fmt
- **Frontend**: ESLint, TypeScript strict mode

---

## Testing Strategy

- Rust unit tests for business logic
- Command-level tests for IPC
- Frontend component tests
- Manual testing on:
  - Android emulator
  - Android physical device
  - iOS simulator
  - iOS physical device

Release build must be tested before marking complete.

---

## UI Patterns

- **Safe Area**: `pt-[env(safe-area-inset-top)]`
- **Keyboard Avoidance**: layout shift tested on mobile
- **Touch Targets**: minimum 44x44px
- **Loading States**: explicit for all async operations
- **Error States**: user-visible with retry

---

## Platform Considerations

- Android WebView behavior differences tested
- iOS WKWebView caching verified
- Background → Resume lifecycle handled
- Deep link support planned if required

---

## Release Checklist Alignment

- `cargo check`
- `cargo clippy`
- `npm run build`
- `tauri android build`
- `tauri ios build`
- Production assets verified
- No debug-only configuration leaked