# PM Agent - Execution Protocol (Tauri Project)

## Step 0: Prepare
1. **Load Protocols**:
   - `../_shared/multi-review-protocol.md` (Review 1-4)
   - `../_shared/phase-gates.md` (Plan Gate)
   - `../_shared/memory-protocol.md` (CLI mode)

2. **Assess difficulty** — see `../_shared/difficulty-guide.md`
   - Simple: Lightweight plan (3–5 tasks)
   - Medium: Full 4 steps
   - Complex: Full + explicit API & IPC contracts

3. **Clarify requirements** — follow `../_shared/clarification-protocol.md`
   - Identify business logic ambiguity
   - Identify security/auth implications
   - Identify cross-platform implications (Web / Android / iOS)
   - Escalate immediately if HIGH uncertainty

4. **Review Architecture Context**
   - Read `tech-stack.md`
   - Review existing IPC commands
   - Review `src-tauri/capabilities`
   - Identify current plugin usage

5. **Check lessons learned**
   - Read cross-domain section in `../_shared/lessons-learned.md`

6. **Memory**
   - Create `session-pm-{id}.md`

---

## Step 1: Analyze Requirements

- Parse request into:
  - Functional requirements
  - Non-functional requirements (performance, security, platform)
- Identify implicit requirements
- Identify edge cases
- Identify impact areas:
  - Frontend UI
  - IPC layer
  - Rust backend
  - Database
  - Mobile native wrapper
- List assumptions clearly

---

## Step 2: Design Architecture

- **Define Boundaries**
  - What runs in frontend?
  - What runs in Rust?
  - What requires native plugin?

- **Define IPC Contracts**
  - Command name
  - Request schema
  - Response schema
  - Standardized error structure

- **Define Data Models**
  - Tables
  - Relationships
  - Indexes
  - Migration impact

- **Security Design**
  - Input validation layer
  - Capability requirements
  - Secret handling strategy
  - Auth strategy

- **Platform Strategy**
  - Android-specific considerations
  - iOS-specific considerations
  - Web fallback (if applicable)

- **Performance Strategy**
  - Heavy logic moved to Rust
  - Async model
  - Memory impact

- **Observability**
  - Logging strategy
  - Error reporting
  - Release diagnostics

---

## Step 3: Decompose Tasks

- Break into agent-specific tasks:
  - Frontend Agent
  - Backend Agent
  - Mobile Agent
- Each task must include:
  - Title
  - Description
  - Acceptance Criteria (measurable)
  - Priority Tier (1, 2, 3…)
  - Complexity (Low / Medium / High / Very High)
  - Dependencies

- Rules:
  - IPC contracts must be defined before frontend/mobile work
  - Database schema must be finalized before API implementation
  - Security tasks cannot be deferred
  - Minimize cross-agent blocking

- Save to:
  - `.agent/plan.json`
  - `.agent/brain/current-plan.md`

---

## Step 4: Validate Plan

- Check completeness:
  - [ ] All requirements covered?
  - [ ] All edge cases considered?
- Check independence:
  - [ ] Can tasks run in parallel safely?
- Check security:
  - [ ] Is validation defined?
  - [ ] Are capabilities explicitly listed?
- Check contract-first:
  - [ ] Are IPC/API contracts defined before UI tasks?
- Check production readiness:
  - [ ] Release build considered?
  - [ ] Logging included?
  - [ ] Performance constraints addressed?

- Output in `task-board.md` format for orchestrator compatibility.

---

## Step 5: Finalize

- Present structured plan to User
- Request confirmation before execution
- Write `result-pm-{id}.md`

---

## On Error

See `resources/error-playbook.md`