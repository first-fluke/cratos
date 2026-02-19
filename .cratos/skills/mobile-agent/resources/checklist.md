# Mobile Agent - Self-Verification Checklist

Run strictly before completion.

Build & Runtime
- [ ] Application builds successfully (tauri android build / tauri ios build).
- [ ] Release build tested (not only debug).
- [ ] No crash on startup (Splash screen transitions correctly).
- [ ] Permissions requested contextually (not all at launch).
- [ ] App resumes correctly from background state.
- [ ] Deep links open correct route.

Architecture
- [ ] Clear separation: frontend (UI) ↔ Rust commands (business logic).
- [ ] All #[tauri::command] functions validate input.
- [ ] Shared TypeScript types match Rust response structures.
- [ ] Centralized error mapping (Rust error → standardized frontend error).
- [ ] No business logic inside UI components.
- [ ] State management consistent (e.g., Zustand/Redux/Signals — single strategy only).

Native Capabilities
- [ ] Plugins registered in lib.rs (.plugin(tauri_plugin_xyz::init())).
- [ ] Capabilities defined in src-tauri/capabilities.
- [ ] Sensitive data stored using secure storage plugin.
- [ ] File system access scoped and validated.
- [ ] Fallback for APIs not available on Web (if checking window.__TAURI__).

Security
- [ ] IPC commands restricted (no unnecessary public commands).
- [ ] Input sanitized before filesystem or DB usage.
- [ ] No secrets exposed in frontend bundle.
- [ ] CSP (Content Security Policy) configured.
- [ ] Production build disables devtools.

Performance
- [ ] No heavy JS computation blocking UI thread.
- [ ] Long-running Rust tasks executed asynchronously.
- [ ] Images optimized for mobile data/memory.
- [ ] Scroll performance is 60fps.
- [ ] Cold start time measured and acceptable.

Logging & Observability
- [ ] Structured logging enabled in Rust.
- [ ] Frontend error logging implemented.
- [ ] Crash reporting integrated.
- [ ] Log level differs between dev and production.

Testing
- [ ] Rust unit tests for business logic.
- [ ] Command-level tests (IPC boundary).
- [ ] Frontend component tests.
- [ ] Manual test on physical Android device.
- [ ] Manual test on physical iOS device.