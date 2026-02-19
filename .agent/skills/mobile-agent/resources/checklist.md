# Mobile Agent - Self-Verification Checklist

Run strictly before completion.

## Build & Runtime
- [ ] Application builds successfully (`tauri android build` / `tauri ios build`).
- [ ] No crash on startup (Splash screen transitions correctly).
- [ ] Permissions requested at appropriate time (not all at launch).

## Native Capabilities
- [ ] Plugins registered in `lib.rs` (`.plugin(tauri_plugin_xyz::init())`).
- [ ] Capabilities defined in `src-tauri/capabilities`.
- [ ] Fallback for APIs not available on Web (if checking `window.__TAURI__`).

## UI/UX
- [ ] Touch targets are at least 44x44px.
- [ ] Safe Areas (Notch, Home Indicator) handled using CSS env variables.
- [ ] Input fields don't obscured by keyboard (Keyboard avoidance).
- [ ] Back button (Android) handled correctly.

## Performance
- [ ] No heavy JS computation blocking UI thread.
- [ ] Images optimized for mobile data/memory.
- [ ] Scroll performance is 60fps.
