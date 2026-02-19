---
name: gap_analysis
description: Identify gaps between current and desired states in architecture, process, or implementation.
version: 1.0.0
triggers:
  - "gap analysis"
  - "analyze gaps"
  - "identify missing features"
  - "compare implementation to design"
  - "find discrepancies"
---

# Gap Analysis

Systematic tool for identifying discrepancies between current implementation/state and desired requirements/design.
Used primarily in the **PLAN** and **REFINE** phases to ensure alignment.

## Role

- Analyze current codebase or system state against requirements.
- Identify missing components, deviations, or technical debt.
- Provide actionable recommendations to bridge the identified gaps.
- Verify improved alignment after changes.

## Core Rules

1.  **Evidence-Based**: All gaps must be supported by specific file paths, code snippets, or configuration values.
2.  **Actionable**: Each gap must have a clear remediation step (e.g., "Implement X", "Refactor Y").
3.  **Scoped**: Focus on the specific domain requested (e.g., Security, Architecture, Feature Parity).
4.  **No Assumptions**: Verify the current state using tools (`view_file`, `grep_search`) before declaring a gap.

## Workflow

### Step 1: Definition of States

Define the "Desired State" (from requirements/docs) and "Current State" (from codebase/runtime).
Use `read_file` to fetch requirements and `search_for_pattern`/`find_symbol` to explore current code.

### Step 2: Gap Identification

Compare the two states. Categorize gaps into:
- **Missing**: Feature exists in design but not in code.
- **Deviation**: Feature exists but behaves differently or has wrong signature.
- **Extra**: Code exists that is not in design (scope creep).

### Step 3: Impact Analysis

Assess the impact of each gap (Blocking, High, Medium, Low).
Determines priority for remediation.

### Step 4: Remediation Plan

Generate a list of tasks to close the gaps.
Output format should be compatible with Task Tracking (e.g., Markdown checklist or JSON).

## Resources

- **Protocol**: `resources/execution-protocol.md` (Detailed checklist and methodology)
- **Examples**: `resources/examples.md` (Common gap patterns)

