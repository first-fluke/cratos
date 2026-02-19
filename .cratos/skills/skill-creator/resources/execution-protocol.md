# Execution Protocol for Skill Creation

This protocol defines the step-by-step process for the `skill-creator` to generate high-quality skills.

## Phase 1: Analysis & Definition

1.  **Identify the Goal**: Extract the core purpose of the requested skill from the user's prompt.
    - *Input*: "Create a skill for database migration."
    - *Goal*: Automate DB migration tasks safely.
2.  **Define Identifiers**:
    - `skill_name`: kebab-case (e.g., `db-migration-agent`).
    - `triggers`: Key phrases (e.g., "migrate db", "schema update").
3.  **Outline Components**:
    - Does it need a complex workflow? -> Yes/No
    - Does it need specific examples? -> Yes/No
    - existing tools to use? -> (e.g., `run_command`, specific CLI tools).

## Phase 2: Scaffolding

1.  **Create Directories**:
    ```bash
    mkdir -p .cratos/skills/<skill_name>/resources
    ```
2.  **Initialize Files**:
    - create `.cratos/skills/<skill_name>/SKILL.md`
    - create `.cratos/skills/<skill_name>/resources/execution-protocol.md` (optional but recommended)
    - create `.cratos/skills/<skill_name>/resources/examples.md`

## Phase 3: Content Generation

1.  **Fill SKILL.md**:
    - Apply `skill-creator/resources/skill-template.md`.
    - populate `Role`, `Core Rules`, and definitions.
    - **CRITICAL**: Ensure `Core Rules` are strict and unambiguous.
2.  **Draft Protocol**:
    - Define granular steps in `execution-protocol.md`.
    - Each step must have a clear "Definition of Done".
3.  **Provide Examples**:
    - In `examples.md`, show 2-3 pairs of "User Input" -> "Agent Action/Output".

## Phase 4: Validation

1.  **Check Consistency**: Do the file paths in `SKILL.md` match the actual created files?
2.  **Check Clarity**: Are the triggers unique enough? Are the rules enforceable?
3.  **User Review**: Present the generated skill structure to the user for confirmation (if interactive).
