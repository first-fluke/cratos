# Frontend Agent - Error Playbook

## 1. Hydration Mismatch
**Symptom**: `Text content does not match server-rendered HTML.`
**Solution**:
- Ensure `window`, `localStorage`, or random data (dates) are accessed only in `useEffect`.
- Use `suppressHydrationWarning` on elements like Timestamp if necessary.
- Use `dynamic(() => import(...), { ssr: false })` for client-only components.

## 2. Tauri IPC Error
**Symptom**: `IPC command not found` or `serialization error`.
**Solution**:
- Check `src-tauri/src/lib.rs` -> `tauri::generate_handler![...]` list.
- Verify argument names match EXACTLY (Rust snake_case vs JS camelCase conversion settings).
- Ensure return types are JSON-serializable in Rust (`#[derive(Serialize)]`).

## 3. Tailwind Styles Not Applying
**Symptom**: Classes appear in DOM but no visual change.
**Solution**:
- Check `tailwind.config.ts` content paths (include `src/**/*.{ts,tsx}`).
- Ensure valid utility names (v4 vs v3 differences).
- Check `globals.css` imports.

## 4. Module Not Found
**Symptom**: `Module not found: Can't resolve '@/...'`
**Solution**:
- Check `tsconfig.json` paths configuration:
  ```json
  "paths": {
    "@/*": ["./src/*"]
  }
  ```
