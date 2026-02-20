---
description: Thorough version of coordinate - high-quality development workflow with 11 review steps out of 17
---

# MANDATORY RULES — VIOLATION IS FORBIDDEN

- **Response language follows `language` setting in `.agent/config/user-preferences.yaml` if configured.**
- **NEVER skip steps.** Execute from Step 0 in order. Explicitly report completion of each step to the user before proceeding to the next.
- **You MUST use MCP tools throughout the entire workflow.** This is NOT optional.
  - Use code analysis tools (`get_symbols_overview`, `find_symbol`, `find_referencing_symbols`, `search_for_pattern`) for code exploration.
  - Use memory tools (read/write/edit) for progress tracking.
  - Memory path: configurable via `memoryConfig.basePath` (default: `.serena/memories`)
  - Tool names: configurable via `memoryConfig.tools` in `mcp.json`
  - Do NOT use raw file reads or grep as substitutes. MCP tools are the primary interface for code and memory operations.
- **Read the workflow-guide BEFORE starting.** Read `.cratos/skills/workflow-guide/SKILL.md` and follow its Core Rules.
- **Follow the context-loading guide.** Read `.cratos/skills/_shared/context-loading.md` and load only task-relevant resources.

---

## Phase 0: Initialization (DO NOT SKIP)

1. Read `.cratos/skills/workflow-guide/SKILL.md` and confirm Core Rules.
2. Read `.cratos/skills/_shared/context-loading.md` for resource loading strategy.
3. Read `.cratos/skills/_shared/memory-protocol.md` for memory protocol.
4. Read `.cratos/skills/_shared/multi-review-protocol.md` (11 review guides)
5. Read `.cratos/skills/_shared/quality-principles.md` (4 principles)
6. Read `.cratos/skills/_shared/phase-gates.md` (gate definitions)
7. Use memory write tool to create session files:
   - Create `orchestrator-session.md` with: session start time, user request summary, workflow version (pro)
   - Create `task-board.md` with the following **table format** (required for dashboard sync):
     ```markdown
     # Task Board
     ## Session: {session-id}
     ## Status: RUNNING

     | Agent        | Status  | Task               |
     | ------------ | ------- | ------------------ |
     | {agent-name} | pending | {task-description} |
     ```
   - Valid statuses: `pending`, `running`, `completed`, `failed`, `blocked`
   - **WARNING**: Do NOT use checklist or bullet format. The dashboard parser (`IF()`) requires pipe-delimited table rows.

---

## Phase 1: PLAN (Steps 1-4)

### Step 1: Create Plan & Review
// turbo
Activate PM Agent to execute Steps 1-4 (Creation + 3 Reviews):

1. Read `.cratos/skills/pm-agent/SKILL.md` and follow its instructions.
2. Analyze requirements using MCP code analysis tools.
3. Execute Step 1: Create Plan using `.cratos/skills/pm-agent/resources/plan_template.json`.
4. Save plan to `.agent/plan.json` and use memory write tool to record plan completion.
5. Execute Step 2 (Completeness Review): Ensure all requirements are fully mapped.
6. Execute Step 3 (Meta Review): Self-verify if the review was sufficient.
7. Execute Step 4 (Over-Engineering Review): Check for unnecessary complexity (MVP focus).

### Step 2: Plan Review (Completeness)
- **Activated inline**: Ensure requirements are fully mapped.

### Step 3: Review Verification (Meta Review)
- **Activated inline**: Self-verify if the review was sufficient.

### Step 4: Over-Engineering Review (Simplicity)
- **Activated inline**: Check for unnecessary complexity (MVP focus).

### PLAN_GATE
- [ ] Plan documented in `.agent/plan.json`
- [ ] Assumptions listed
- [ ] Alternatives considered
- [ ] Over-engineering review done
- [ ] **User confirmation**

**Gate failure → Return to Step 1**

> **On GATE pass**: Use memory edit tool to update `task-board.md` — set pm-agent status to `completed`. Update `orchestrator-session.md` with Phase 1 completion.

---

## Phase 2: IMPL (Step 5)

### Step 5: Implementation
// turbo
Spawn Implementation Agents (Backend/Frontend/Mobile) in parallel.
Command:
```bash
oh-my-ag agent:spawn backend-agent "Implement backend tasks per plan..." session-id -w ./backend &
oh-my-ag agent:spawn frontend-agent "Implement frontend tasks per plan..." session-id -w ./frontend &
wait
```

### IMPL_GATE
- [ ] Build succeeds
- [ ] Tests pass
- [ ] Only planned files modified

**Gate failure → Re-run Step 5**

> **On GATE pass**: Use memory edit tool to update `task-board.md` — set impl agent(s) status to `completed`. Update `orchestrator-session.md` with Phase 2 completion.

### Monitor Implementation Progress

While agents are running:
- Use memory read tool to poll `progress-{agent}.md` files for status updates.
- Use memory edit tool to update `task-board.md` with agent status changes (`running`, `completed`, `failed`).
- Use MCP code analysis tools to verify implementation alignment if needed.
- Watch for: completion, failures, crashes. Re-spawn failed agents (max 2 retries).

---

## Phase 3: VERIFY (Steps 6-8)

### Step 6-8: QA Verification
// turbo
Spawn QA Agent to execute Steps 6-8.
Command: `oh-my-ag agent:spawn qa-agent "Execute Phase 3 Verification. Step 6: Alignment Review. Step 7: Security/Bug Review (npm audit, OWASP). Step 8: Improvement/Regression Review." session-id`

### Step 6: Alignment Review
- **Delegated to QA Agent**: Compare implementation vs plan.

### Step 7: Security/Bug Review (Safety)
- **Delegated to QA Agent**: Check for vulnerabilities (Safety).

### Step 8: Improvement Review (Regression Prevention)
- **Delegated to QA Agent**: Run regression tests.

### VERIFY_GATE
- [ ] Implementation = Requirements
- [ ] CRITICAL count: 0
- [ ] HIGH count: 0
- [ ] No regressions

**Gate failure → Return to Step 5 (fix implementation)**

> **On GATE pass**: Use memory edit tool to update `task-board.md` — set qa-agent status to `completed`. Update `orchestrator-session.md` with Phase 3 completion.

---

## Phase 4: REFINE (Steps 9-13)

### Step 9-13: Deep Refinement
// turbo
Spawn Debug Agent (or Senior Dev Agent) to execute Steps 9-13.
Command: `oh-my-ag agent:spawn debug-agent "Execute Phase 4 Refine. Step 9: Split large files. Step 10: Integration check. Step 11: Side Effect analysis (find_referencing_symbols). Step 12: Consistency review. Step 13: Cleanup dead code." session-id`

### Step 9: Split Large Files/Functions
- **Delegated to Debug Agent**: Files > 500 lines, Functions > 50 lines.

### Step 10: Integration/Reuse Review (Reusability)
- **Delegated to Debug Agent**: Check for duplicate logic.

### Step 11: Side Effect Review (Cascade Impact)
- **Delegated to Debug Agent**: Analyze impact scope.

### Step 12: Full Change Review (Consistency)
- **Delegated to Debug Agent**: Review naming and style.

### Step 13: Clean Up Unused Code
- **Delegated to Debug Agent**: Remove newly created dead code.

### REFINE_GATE
- [ ] No large files/functions
- [ ] Integration opportunities captured
- [ ] Side effects verified
- [ ] Code cleaned

**Skip conditions**: Simple tasks < 50 lines

> **On GATE pass**: Use memory edit tool to update `task-board.md` — set debug-agent status to `completed`. Update `orchestrator-session.md` with Phase 4 completion.

---

## Phase 5: SHIP (Steps 14-17)

### Step 14-17: Final QA & Deployment Readiness
// turbo
Spawn QA Agent to execute Steps 14-17.
Command: `oh-my-ag agent:spawn qa-agent "Execute Phase 5 Ship. Step 14: Quality Review (lint/coverage). Step 15: UX Flow Verification. Step 16: Related Issues Review. Step 17: Deployment Readiness." session-id`

### Step 14: Code Quality Review
- **Delegated to QA Agent**: Lint, Types, Coverage.

### Step 15: UX Flow Verification
- **Delegated to QA Agent**: User journey check.

### Step 16: Related Issues Review (Cascade Impact 2nd)
- **Delegated to QA Agent**: Final impact check.

### Step 17: Deployment Readiness Review (Final)
- **Delegated to QA Agent**: Secrets, Migrations, checklist.

### SHIP_GATE
- [ ] Quality checks pass
- [ ] UX verified
- [ ] Related issues resolved
- [ ] Deployment checklist complete
- [ ] **User final approval**

> **On GATE pass**: Use memory edit tool to update `task-board.md` — set all agent statuses to `completed`, session status to `COMPLETED`. Update `orchestrator-session.md` with final completion.

---

## Phase 6: CLEANUP (Steps 18)

### Step 18: Cleanup Memories
Manually (or via script) remove temporary memory files to keep the workspace clean.
- **Keep**: `session-*.md`, `result-*.md`, `.agent/plan.json`
- **Delete**: `progress-*.md`

---

## Review Steps Summary

| Phase   | Steps | Mode             | Agent       | Perspective                       |
| ------- | ----- | ---------------- | ----------- | --------------------------------- |
| PLAN    | 1-4   | **Activate**     | PM Agent    | Completeness, Meta, Simplicity    |
| IMPL    | 5     | Spawn (parallel) | Dev Agents  | Implementation                    |
| VERIFY  | 6-8   | Spawn            | QA Agent    | Alignment, Safety, Regression     |
| REFINE  | 9-13  | Spawn            | Debug Agent | Reusability, Cascade, Consistency |
| SHIP    | 14-17 | Spawn            | QA Agent    | Quality, UX, Cascade 2nd, Deploy  |
| CLEANUP | 18    | Coordinator      | —           | Workspace Hygiene                 |

> **Activate** = 현재 에이전트가 해당 Skill을 직접 읽고 인라인으로 실행 (외부 프로세스 없음)
> **Spawn** = `oh-my-ag agent:spawn`으로 별도 CLI 프로세스 실행 (모니터링 가능)

**Total 11 review steps + Cleanup → High quality guaranteed**
