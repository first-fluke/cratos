# Mobile Agent - Execution Protocol (Tauri Mobile)

## Step 0: Prepare

1. **Assess difficulty** — see `../_shared/difficulty-guide.md`
   - **Simple**: Skip to Step 3
   - **Medium**: All 4 steps
   - **Complex**: All steps + mid-checkpoint

2. **Load Protocols**:
   - `../_shared/multi-review-protocol.md`
   - `../_shared/quality-principles.md`
   - `../_shared/phase-gates.md`
   - `../_shared/memory-protocol.md` (CLI mode)

3. **Context Review**:
   - `get_symbols_overview("src-tauri/capabilities")`
   - Check `Cargo.toml` for plugin dependencies
   - Review `tauri.conf.json`
   - Inspect `src-tauri/src/lib.rs` for registered commands

4. **Memory**:
   - Create `session-mobile-{id}.md` (if leading)
   - Or update `progress-{id}.md`

5. **Clarify Requirements**:
   - Does feature require native Rust code?
   - Does it require platform-specific behavior (Android/iOS)?
   - Are new permissions required?
   - Any security implications?

**⚠ Escalate early if IPC contract or security requirements are unclear.**

---

## Step 1: Analyze

- Read task requirements carefully
- Determine:
  - Frontend-only change?
  - Rust command required?
  - New plugin required?
- Identify affected layers:
  - UI (frontend)
  - IPC (`invoke`)
  - Rust business logic
  - Native capability
- Review:
  - `src-tauri/gen/android`
  - `src-tauri/gen/apple`
- Check for existing similar commands or patterns
- List assumptions

---

## Step 2: Plan

- **IPC Contract**:
  - Define request/response structure
  - Ensure TypeScript types match Rust structs
  - Define standardized error format

- **Rust Layer**:
  - Define `#[tauri::command]`
  - Plan async strategy (`tokio::spawn` if needed)
  - Avoid blocking operations

- **Plugins**:
  - Identify required `@tauri-apps/plugin-*`
  - Add crate dependency plan

- **Permissions / Capabilities**:
  - Update `src-tauri/capabilities/mobile.json`
  - Confirm strict capability compliance

- **Security**:
  - Validate input
  - Sanitize file paths
  - Avoid exposing secrets to frontend

- **UI Plan**:
  - Safe Area handling
  - Touch target size
  - Loading and error states
  - Platform-specific UI differences

- **Performance Plan**:
  - Move heavy logic to Rust
  - Avoid blocking JS thread
  - Consider memory footprint

---

## Step 3: Implement

1. **Rust Layer**
   - Add dependency to `Cargo.toml`
   - Implement command in `src-tauri/src/lib.rs`
   - Register plugin if needed
   - Add proper error handling (no `unwrap()`)

2. **Capabilities**
   - Update `mobile.json`
   - Confirm permissions list correct
   - Rebuild after changes

3. **Frontend Integration**
   - Install required NPM package
   - Implement `invoke()` with proper typing
   - Handle loading + error states
   - Validate response before rendering

4. **UI**
   - Implement responsive layout
   - Handle `safe-area-inset-*`
   - Ensure keyboard avoidance works

5. **Logging**
   - Add structured logging in Rust
   - Log IPC errors on frontend (no secrets)

---

## Step 3.5 (Complex Only)

- Update `progress-{id}.md`
- Verify IPC contract still valid
- Confirm no architectural drift

---

## Step 4: Verify

- Run:
  - `npm run tauri android dev`
  - `npm run tauri ios dev`

- Test on:
  - Android emulator + physical device
  - iOS simulator + physical device

- Validate:
  - Permissions prompt timing
  - Background → resume behavior
  - No Rust panic
  - No white screen in release build
  - CSP does not block scripts
  - Performance acceptable (no JS blocking)

- Run:
  - `cargo check`
  - `cargo fmt`
  - `cargo clippy`

- Confirm:
  - Strict capability compliance
  - No devtools enabled in production
  - Release build tested

- **Memory**:
  - Write `result-{id}.md` with completion status

---

## On Error

See `resources/error-playbook.md`