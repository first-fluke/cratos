# Frontend Agent - Self-Verification Checklist (Tauri + Next.js)

Run each check before confirming task completion.

## Tauri IPC
- [ ] `invoke` commands use literal strings matching Rust command names.
- [ ] Arguments passed to `invoke` match Rust struct fields exactly (snake_case vs camelCase handled?).
- [ ] Error handling wrapped in `try/catch` with user-facing feedback (Toast/Alert).

## Next.js / React
- [ ] No Hydration Errors (use `useEffect` for browser-only APIs like `window`).
- [ ] `use client` directive present on interactive components.
- [ ] `useParams` / `useSearchParams` used correctly for routing.

## UI/UX (Shadcn + Tailwind)
- [ ] Used `shadcn/ui` components from `components/ui`.
- [ ] Responsive design verified (Mobile/Desktop views).
- [ ] Dark mode support verified (using Tailwind `dark:` prefix).
- [ ] Accessibility: `aria-label` on icon buttons, semantic HTML.

## Performance
- [ ] Large dependencies loaded lazily (`next/dynamic`).
- [ ] Images optimized (using `next/image` where possible, or asset handling).
- [ ] Minimize re-renders (use `useMemo` / `useCallback` appropriately).
