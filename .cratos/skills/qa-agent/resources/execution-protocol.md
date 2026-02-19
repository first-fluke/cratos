# QA Agent - Execution Protocol

## Step 0: Prepare

1. **Assess difficulty** — see `../_shared/difficulty-guide.md`
   - Simple: Quick security + quality check
   - Medium: Full review
   - Complex: Full review + prioritized deep audit

2. **Check lessons**
   - Read QA section in `../_shared/lessons-learned.md`

3. **Clarify requirements**
   - Identify security/auth implications
   - Identify IPC boundary impact
   - Identify platform-specific risks (Android/iOS)

4. **Scope context**
   - Determine impacted layers:
     - Frontend (React)
     - IPC layer (`invoke`)
     - Rust backend
     - Capabilities
     - Native wrapper

5. **Memory**
   - Create `session-qa-{id}.md` (if leading)

---

## Step 1: Scope

- Identify review type:
  - New feature verification
  - Security audit
  - Performance review
  - Full system audit

- Map impacted files:
  - `src-tauri/src`
  - `src-tauri/capabilities`
  - Frontend feature modules
  - `tauri.conf.json`

- Use search patterns:

  - `search_for_pattern("unwrap()")`
  - `search_for_pattern("expect(")`
  - `search_for_pattern("invoke(")`
  - `search_for_pattern("innerHTML")`
  - `search_for_pattern("fs::")`
  - `search_for_pattern("password")`
  - `search_for_pattern("SECRET")`

---

## Step 2: Audit (Priority Order)

### 1. Security (CRITICAL)

- Validate IPC input sanitization
- Ensure no `unwrap()` in production logic
- Verify standardized error handling
- Confirm capabilities strictly defined
- Check file system access scoping
- Verify no secrets in frontend bundle
- Check CSP configuration
- Verify iOS ATS compliance
- Confirm devtools disabled in release build

### 2. IPC Boundary Integrity

- TypeScript types match Rust structs
- All commands registered in `lib.rs`
- Error format consistent across commands
- No unnecessary public commands exposed

### 3. Performance

- Heavy computation moved to Rust
- No blocking synchronous JS
- No excessive re-renders
- Release build tested
- Cold start time reasonable

### 4. Stability

- Background → resume behavior verified
- No Rust panic in logs
- No white screen in release build
- Android WebView tested
- iOS WKWebView tested

### 5. Code Quality

- `cargo clippy` clean
- `cargo fmt` clean
- ESLint passes
- Tests exist for business logic
- IPC commands have at least minimal coverage

---

## Step 3: Execute Verification

- Run:
  - `cargo check`
  - `cargo clippy`
  - `cargo test`
  - `npm test`
  - `cargo audit`
  - `npm audit`

- Build:
  - `tauri android build`
  - `tauri ios build`

- Test:
  - Emulator
  - Physical Android device
  - Physical iOS device

- Inspect logs:
  - `adb logcat`
  - Xcode device logs

---

## Step 4: Report

Generate structured report:

- Overall Status:
  - PASS
  - WARNING
  - FAIL

- Findings grouped by severity:
  - CRITICAL
  - HIGH
  - MEDIUM
  - LOW

Each finding must include:
- File:line reference
- Description
- Risk explanation
- Recommended remediation
- Reproducibility confirmed

---

## Step 5: Self-Verify

- Ensure no false positives
- Confirm each issue reproducible
- Confirm remediation technically valid
- Run `../_shared/common-checklist.md`

---

## On Error

See `resources/error-playbook.md`