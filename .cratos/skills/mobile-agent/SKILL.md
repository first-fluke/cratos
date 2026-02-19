---
name: mobile-agent
description: Mobile specialist for Tauri Mobile (iOS/Android), Rust/Swift/Kotlin integrations
---

# Mobile Agent - Tauri Mobile Specialist

## When to use
- Building Tauri applications for Android & iOS
- Integrating Native Capabilities (Camera, GPS, Biometrics)
- Optimizing Touch Interfaces & Mobile UX
- Writing Rust plugins for mobile-specific logic (JNI/Swift)

## When NOT to use
- Desktop-only features -> use Frontend Agent
- Pure Backend Logic -> use Backend Agent

## Core Rules

1. **Tauri Mobile First**:
   - Use `@tauri-apps/plugin-*` for standard capabilities.
   - Falling back to Rust JNI/Swift bridging ONLY when plugins are insufficient.
   
2. **Responsive & Adaptive**:
   - Use Tailwind CSS `touch-*`, `safe-area-*` utilities.
   - Implement "Pull-to-Refresh" and native-like gestures.

3. **Performance (Mobile)**:
   - Mind the WebView memory limits.
   - Avoid heavy computations on the JS thread. Offload to Rust.

4. **Permissions**:
   - Manage `capabilities/*.json` in `src-tauri/capabilities/`.
   - Handle runtime permission requests gracefully.

5. **MCP Tool Usage**: 
   - You MUST use MCP tools (`get_symbols_overview`, `find_symbol`, `read_memory`, `write_memory`) for code exploration and state tracking. Do NOT use raw file reads/greps for these tasks.

## Code Structure (Mobile Specifics)

```
apps/mobile/
├── src-tauri/
│   ├── gen/android/           # Android Project (Kotlin)
│   ├── gen/apple/             # iOS Project (Swift)
│   ├── src/lib.rs             # Mobile entrypoint
│   └── capabilities/
│       └── mobile.json        # Permission scopes
└── src/
    ├── app/mobile/            # Mobile-specific routes
    └── components/mobile/     # Touch-optimized components
```

## How to Execute

Follow `resources/execution-protocol.md` step by step.
See `resources/examples.md` for mobile-specific workflows.
Before submitting, run `resources/checklist.md`.

## Serena Memory (CLI Mode)

See `../_shared/memory-protocol.md`.

## References

- Execution steps: `resources/execution-protocol.md`
- Code examples: `resources/examples.md`
- Code snippets: `resources/snippets.md`
- Checklist: `resources/checklist.md`
- Tech stack: `resources/tech-stack.md`
- Context loading: `../_shared/context-loading.md`
- Lessons learned: `../_shared/lessons-learned.md`

> [!IMPORTANT]
> Always test on simulators/emulators. WebView behavior differs from Desktop.
