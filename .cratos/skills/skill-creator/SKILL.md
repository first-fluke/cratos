---
name: skill-creator
description: Meta-skill for automatically generating new skills with clear, actionable, and definite behaviors.
version: 1.0.0
triggers:
  - "스킬 생성", "create skill", "new skill"
  - "스킬 만들어줘", "make a skill"
  - "skill-creator"
---

# Skill Creator

A specialized agent for creating new skills within the `.cratos/skills` directory.
Ensures all generated skills follow the strict Cratos standards for clarity, actionability, and consistency.

## Role

- Analyze user requirements to define the scope and purpose of a new skill.
- Generate a comprehensive `SKILL.md` file following the standard template.
- Create necessary supporting resources (protocols, examples) in a `resources/` subdirectory.
- Validate that the new skill has clear triggers, roles, and actionable rules.

## Core Rules

1.  **Standard Structure**: All skills MUST have a `SKILL.md` with YAML frontmatter (name, description, version, triggers).
2.  **Clear Actions**: The `SKILL.md` must define *what* the skill does, not just *how*. Avoid vague descriptions.
3.  **Mandatory Sections**:
    - **Role/Objective**: What problem does this skill solve?
    - **Core Rules**: Inviolable rules for the skill's execution.
    - **Workflow/Execution**: Step-by-step process or link to an execution protocol.
4.  **Resource Isolation**: Complex logic, prompts, or long protocols MUST be placed in a `resources/` subdirectory, not the main `SKILL.md`.
5.  **Validation**: A skill is only "complete" when it has a valid `SKILL.md` and all referenced resources exist.

## Workflow

1.  **Requirement Analysis**: Understand the user's need. What is the skill's name? What are its triggers? What is its primary function?
2.  **Scaffold Directory**: Create `.cratos/skills/<skill-name>/resources`.
3.  **Draft SKILL.md**: Use `resources/skill-template.md` to create the main definition file.
4.  **Create Resources**:
    - `execution-protocol.md`: Detailed steps for the skill to follow.
    - `examples.md`: Few-shot examples for the model.
5.  **Review**: Verify against the `Core Rules` and ensure the skill is actionable.

## References

- Template: `resources/skill-template.md`
- Protocol: `resources/execution-protocol.md`
- Examples: `resources/examples.md`
