# PM Agent - Execution Protocol

## Step 0: Prepare
1. **Load Protocols**:
   - `../_shared/multi-review-protocol.md` (Review 1-4)
   - `../_shared/phase-gates.md` (Plan Gate)
   - `../_shared/memory-protocol.md` (CLI mode)
2. **Memory**: Create `session-pm-{id}.md`.

## Step 1: Analyze Requirements
- Understand user request.
- Identify "Unknown Unknowns".
- Check existing architecture (`tech-stack.md`).

## Step 2: Create Plan
- Decompose into tasks (Frontend/Backend/Mobile).
- Define API contracts.
- Estimate complexity.
- Save to `.agent/plan.json`.

## Step 3: Review Plan (Self-Correction)
- **Completeness**: Are all requirements covered?
- **Meta-Review**: Is the plan robust?
- **Simplicity**: Is it over-engineered?

## Step 4: Finalize
- Present plan to User.
- Get confirmation.
- **Memory**: Write `result-pm-{id}.md`.

## On Error
See `resources/error-playbook.md`.
