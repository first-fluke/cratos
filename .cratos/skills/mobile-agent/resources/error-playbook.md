# Mobile Agent - Error Recovery Playbook (Tauri)

When you encounter a failure, find the matching scenario and follow the recovery steps.
Do NOT stop until all relevant steps are exhausted.

---

## Android Build Failure (Gradle)

- [ ] Check `src-tauri/gen/android/build.gradle.kts`
- [ ] Run `cd src-tauri/gen/android && ./gradlew clean`
- [ ] Verify Kotlin and SDK versions match Tauri requirements
- [ ] Ensure NDK version matches `tauri.conf.json`
- [ ] Delete `src-tauri/gen/android/.gradle` and rebuild
- [ ] Confirm no dependency version conflicts

---

## iOS Build / Signing Failure

- [ ] Open `src-tauri/gen/apple/Runner.xcodeproj` in Xcode
- [ ] Verify correct Team selected
- [ ] Verify Bundle Identifier matches provisioning profile
- [ ] Check minimum iOS deployment target
- [ ] Clean build folder in Xcode and rebuild
- [ ] Confirm ATS settings allow required network domains

---

## Rust Compilation Error

- [ ] Run `cargo check`
- [ ] Run `cargo build`
- [ ] Ensure all plugins are compatible with current Tauri version
- [ ] Verify feature flags in `Cargo.toml`
- [ ] Confirm no missing trait imports
- [ ] Fix warnings that may become errors in release mode

---

## Rust Panic / App Crash

- [ ] Check device logs (`adb logcat` or Xcode console)
- [ ] Ensure no `unwrap()` on fallible results
- [ ] Replace `unwrap()` with proper error handling
- [ ] Validate all command inputs before processing
- [ ] Ensure async functions use `tokio::spawn` if long-running
- [ ] Confirm no blocking sync code inside async context

---

## Command Not Allowed (Capability Error)

- [ ] Check `src-tauri/capabilities/mobile.json`
- [ ] Ensure required plugin permission exists
- [ ] Confirm capability file is referenced in `tauri.conf.json`
- [ ] Rebuild application (capabilities baked at build time)
- [ ] Verify command name matches exactly in `#[tauri::command]`

---

## IPC / invoke() Failure (Frontend ↔ Rust)

- [ ] Ensure TypeScript request shape matches Rust struct
- [ ] Validate response serialization (`serde::Serialize`)
- [ ] Confirm command registered in `lib.rs`
- [ ] Catch errors in frontend and log full payload
- [ ] Test command independently with minimal input

---

## White Screen on Launch

- [ ] Inspect WebView console (Safari/Chrome DevTools)
- [ ] Check for JS runtime errors
- [ ] Verify production assets correctly bundled
- [ ] Confirm CSP does not block scripts
- [ ] Ensure frontend routing works in file protocol
- [ ] Clear WebView cache and retry

---

## Async / Deadlock Issue

- [ ] Ensure no blocking I/O in main thread
- [ ] Move heavy computation to Rust side
- [ ] Use async database drivers properly
- [ ] Confirm no mutex deadlock in shared state
- [ ] Avoid long synchronous loops in frontend

---

## Secure Storage / File Access Error

- [ ] Ensure secure storage plugin registered
- [ ] Validate storage permission granted
- [ ] Confirm file path scoped within allowed directories
- [ ] Check platform-specific storage differences (iOS sandbox vs Android)

---

## Network / API Error

- [ ] Confirm backend reachable from mobile device
- [ ] Verify HTTPS certificate validity (iOS ATS)
- [ ] Check CORS if using dev server
- [ ] Log raw API response before parsing
- [ ] Ensure timeout configured appropriately

---

## Background / Resume Crash

- [ ] Verify app handles lifecycle resume event
- [ ] Rehydrate state safely on resume
- [ ] Ensure no stale async task accessing destroyed state
- [ ] Test background → foreground transition multiple times

---

## Performance Degradation

- [ ] Check for heavy JS computation blocking UI
- [ ] Profile Rust side for long tasks
- [ ] Ensure images optimized
- [ ] Monitor memory usage on device
- [ ] Confirm no event listener leaks

---

## Logging & Diagnostics

- [ ] Enable structured logging in Rust
- [ ] Log all IPC errors with payload
- [ ] Differentiate dev vs production log level
- [ ] Reproduce issue in release build (not only debug)

---

## Recovery Principles

- [ ] After 3 failures with same approach → change strategy
- [ ] If blocked after 5 iterations → document state and mark `Status: blocked`
- [ ] Never suppress errors without understanding root cause
- [ ] Never expose secrets in frontend logs