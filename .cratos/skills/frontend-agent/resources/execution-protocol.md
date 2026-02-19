# Frontend Agent - Execution Protocol

## Step 0: Prepare
1. **Load Protocols**:
   - `../_shared/multi-review-protocol.md`
   - `../_shared/quality-principles.md`
   - `../_shared/phase-gates.md`
   - `../_shared/memory-protocol.md` (CLI mode)
2. **Context**: `get_symbols_overview("src/components")`, check `src-tauri/src/lib.rs` for available commands.
3. **Memory**: Create `session-frontend-{id}.md` (if leading) or update `progress-{id}.md`.

## Step 1: Analyze
- Check existing `components` vs needed custom components.
- Verify if Rust `#[tauri::command]` exists for required logic.

## Step 2: Plan
- **IPC**: Define `invoke` signatures (Input/Output types).
- **State**: Decide between Local (useState), Global (Zustand), or Server (Query).
- **UI**: Wireframe component hierarchy.

## Step 3: Implement
1. **Types**: Define TS interfaces for Rust structs.
2. **Hooks**: Create custom hooks for Tauri commands.
3. **Components**: Build UI using Tailwind + modular components (see `component-template.tsx`).
4. **Integration**: Connect hooks to UI events.

## Step 4: Verify
- Run `npm run lint`.
- Build check: `npm run build` (Next.js build).
- Verify against `resources/checklist.md`.
- **Memory**: Write `result-{id}.md` with completion status.

## On Error
See `resources/error-playbook.md`.
