# Dynamic Context Loading Guide

Agents should not read all resources at once. Instead, load only necessary resources based on task type.
This saves context window and prevents confusion from irrelevant information.

---

## Loading Order (Common to All Agents)

### Always Load (Required)
1. `SKILL.md` — Auto-loaded (provided by Antigravity)
2. `resources/execution-protocol.md` — Execution protocol
3. `../_shared/multi-review-protocol.md` — Multi-step review protocol (for coordinate-pro)
4. `../_shared/quality-principles.md` — Code quality principles
5. `../_shared/phase-gates.md` — Phase gate definitions

### Load at Task Start
3. `../_shared/difficulty-guide.md` — Difficulty assessment (Step 0)

### Load Based on Difficulty
4. **Simple**: Proceed to implementation without additional loading
5. **Medium**: `resources/examples.md` (reference similar examples)
6. **Complex**: `resources/examples.md` + `resources/tech-stack.md` + `resources/snippets.md`

### Load During Execution as Needed
7. `resources/checklist.md` — Load at Step 4 (Verify)
8. `resources/error-playbook.md` — Load only when errors occur
9. `../_shared/common-checklist.md` — For final verification of Complex tasks
10. `../_shared/memory-protocol.md` — CLI mode only

---

## Task Type → Resource Mapping by Agent

### Backend Agent (Rust + Axum + SQLx)

| Task Type               | Required Resources                                   |
| ----------------------- | ---------------------------------------------------- |
| REST API implementation | snippets.md (Axum handler, router) + utoipa guides   |
| Database schema/queries | snippets.md (SQLx migration, query_as!) + schema.sql |
| Authentication          | snippets.md (JWT, Argobap) + security-checklist.md   |
| Business Logic          | examples.md (Service traits, Error handling)         |
| Async Tasks/Queues      | examples.md (Tokio tasks, channels)                  |

### Frontend Agent (Next.js + React + Tailwind + Tauri)

| Task Type              | Required Resources                                            |
| ---------------------- | ------------------------------------------------------------- |
| Desktop UI components  | snippets.md (Shadcn, Tailwind) + resources/cratos-ui-guide.md |
| Tauri Integration      | snippets.md (Tauri commands, events) + tauri-conf.json        |
| State Management       | snippets.md (Zustand/Jotai) + examples.md                     |
| Data Fetching          | snippets.md (TanStack Query, Axios)                           |
| Markdown/TUI Rendering | snippets.md (Markdown preview, syntax highlighting)           |

### Mobile Agent (Tauri Mobile)

| Task Type           | Required Resources                                     |
| ------------------- | ------------------------------------------------------ |
| Mobile UI Layout    | snippets.md (Responsive design, Touch targets)         |
| Native Capabilities | snippets.md (Tauri plugins: Camera, Geo, Notification) |
| Offline/Sync        | examples.md (Local DB, Sync logic)                     |
| Performance         | examples.md (Mobile performance tips)                  |

### Debug Agent

| Task Type         | Required Resources                               |
| ----------------- | ------------------------------------------------ |
| Rust Panic/Error  | resources/rust-debugging.md + common-patterns.md |
| Tauri IPC Issues  | resources/tauri-debugging.md                     |
| UI/Rendering Bugs | common-patterns.md (Frontend)                    |
| Concurrency Bugs  | resources/async-debugging.md                     |
| Security/Safety   | common-patterns.md (Unsafe code, memory safety)  |

### QA Agent

| Task Type        | Required Resources                             |
| ---------------- | ---------------------------------------------- |
| Rust Code Audit  | checklist.md (Clippy, fmt, safety)             |
| Security Review  | checklist.md (OWASP, Dependency audit)         |
| Performance Test | checklist.md (Criterion benchmarks, load test) |
| E2E Testing      | checklist.md (Playwright/Tauri driver)         |

### PM Agent

| Task Type             | Required Resources                                             |
| --------------------- | -------------------------------------------------------------- |
| Architecture Planning | resources/architecture-overview.md + api-contracts/template.md |
| Feature Breakdown     | examples.md (User stories, Acceptance criteria)                |
| Release Planning      | resources/release-checklist.md                                 |

---

## Orchestrator Only: Composing Subagent Prompts

When the Orchestrator composes subagent prompts, reference the mapping above
to include only resource paths matching the task type in the prompt.

```
Prompt composition:
1. Agent SKILL.md's Core Rules section
2. execution-protocol.md
3. Resources matching task type (see tables above)
4. error-playbook.md (always include — recovery is essential)
5. Serena Memory Protocol (CLI mode)
```

This approach avoids loading unnecessary resources, maximizing subagent context efficiency.
