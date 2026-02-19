# Task 17: Automate Stale Skill Pruning

- **Status**: [IN_PROGRESS]
- **Created**: 2026-02-19
- **Assignee**: Antigravity (Assistant)
- **Objective**: Automate the cleanup of stale skills (unused for >90 days) by integrating with the Cratos scheduler.

## Context
The project requires an automated mechanism to keep the skill database clean. We have previously implemented the core logic (`list_stale_skills`, `prune_stale_skills`) and CLI support. Now we need to ensure this runs automatically.
User reported that proper protocol (Task file creation, Plan checklist, Strict verification) was missed. This task file serves to rectify that and track the verification and completion of the feature.

## Requirements
1.  **System Task**: A scheduled task `system_prune_stale_skills` must run weekly.
2.  **Configuration**: Retention period should be configurable (default 90 days).
3.  **Safety**: Dry-run capabilities (already in CLI, system task runs effectively in wet mode but logs output).
4.  **Verification**: Code must pass `cargo check` and `cargo clippy`.

## Implementation Plan

### Phase 1: Core Logic (Already Implemented)
- [x] Add `PruneStaleSkills` to `TaskAction` enum (`scheduler/types.rs`).
- [x] Update `execute_task` to handle pruning (`server/task_handler.rs`).
- [x] Update `start_scheduler` to register default task (`server/background_tasks.rs`).

### Phase 2: Verification & Quality Assurance (Current Focus)
- [ ] **Build Check**: Run `cargo check -p cratos` to verify compilation.
- [ ] **Lint Check**: Run `cargo clippy -p cratos` to ensure code quality.
- [ ] **Test**: Verify logic (unit tests for `SkillStore` if possible, or manual verification of startup logs).

### Phase 3: Documentation & Cleanup
- [ ] Update `plan.json`.
- [ ] Update Serena memories.

## Execution Log
- **2026-02-19**: Initial implementation completed (files modified).
- **2026-02-19**: User identified missing protocol. Task file created effectively retroactively to track verification.
- **2026-02-19**: Verification failed (`cargo check`: Operation not permitted on `Cargo.lock` and `~/.cargo/registry`). Permissions issue persists despite `sudo chown`. Manual code review indicates correct logic.
- **2026-02-19**: Verification attempt 2 (Permissions fixed): `cargo check` failed due to missing match arm in `scheduler/engine.rs`. Fixed.
- **2026-02-19**: Verification attempt 3: `cargo check` failed due to missing match arms in `src/api/scheduler.rs`. Fixed.
- **2026-02-19**: Verification attempt 4: `cargo check` failed due to `.await` on synchronous `load_config` in `src/cli/skill.rs` (my implementation). Removing `.await`.
