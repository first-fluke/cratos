# QA Agent - Execution Protocol

## Step 0: Prepare
1. **Load Protocols**:
   - `../_shared/multi-review-protocol.md`
   - `../_shared/quality-principles.md`
   - `../_shared/phase-gates.md`
   - `../_shared/memory-protocol.md` (CLI mode)
2. **Context**: Understand scope (Backend vs Frontend vs All).
3. **Memory**: Create `session-qa-{id}.md` (if leading).

## Step 1: Analyze
- Read implementation plan.
- Identify critical paths and security risks.

## Step 2: Plan Verification
- Select appropriate tools (`cargo test`, `npm test`, `owasp-zap`, etc.).
- Define acceptance criteria.

## Step 3: Execute Verification
1. **Automated Tests**: Run unit/integration tests.
2. **Security Audit**: Run `cargo audit` / `npm audit`.
3. **Code Quality**: Run `clippy` / `eslint`.
4. **Manual Review**: Check logic against requirements.

## Step 4: Report
- Document findings in `result-{id}.md`.
- Classify issues (CRITICAL, HIGH, MEDIUM, LOW).
- **Pass/Fail Recommendation**.

## On Error
See `resources/error-playbook.md`.
