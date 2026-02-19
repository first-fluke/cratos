# Mobile Agent - Execution Protocol (Tauri Mobile)

## Step 0: Prepare
1. **Load Protocols**:
   - `../_shared/multi-review-protocol.md`
   - `../_shared/quality-principles.md`
   - `../_shared/phase-gates.md`
   - `../_shared/memory-protocol.md` (CLI mode)
2. **Context**: `get_symbols_overview("src-tauri/capabilities")`, check `Cargo.toml` for plugins.
3. **Memory**: Create `session-mobile-{id}.md` (if leading) or update `progress-{id}.md`.

## Step 1: Analyze
- Check if requested features need native code (Rust/Kotlin/Swift) or just JS.
- Review `src-tauri/gen/android` and `src-tauri/gen/apple` status.

## Step 2: Plan
- **Plugins**: Identify needed Tauri plugins (`@tauri-apps/plugin-*`).
- **Permissions**: Define capabilities in `src-tauri/capabilities/mobile.json`.
- **UI**: Plan Safe Area handling and Touch gestures.

## Step 3: Implement
1. **Rust Plugin**: Add crate to `Cargo.toml`, register in `src-tauri/src/lib.rs`.
2. **JS Binding**: Install NPM package, invoke via `import { ... } from '@tauri-apps/plugin-...'`.
3. **Capabilities**: Update JSON config to allow plugin commands.
4. **UI**: Implement responsive layout with `safe-area-inset-*` (see `mobile-page-template.tsx`).

## Step 4: Verify
- Run `npm run tauri android dev` or `npm run tauri ios dev`.
- Verify on Emulator/Simulator (not just browser).
- Check permissions prompt behavior.
- **Memory**: Write `result-{id}.md` with completion status.

## On Error
See `resources/error-playbook.md`.
