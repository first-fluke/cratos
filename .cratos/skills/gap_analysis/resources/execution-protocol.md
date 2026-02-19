# Execution Protocol for Gap Analysis

## 1. Input Analysis
- **Goal**: Understand what is being compared.
- **Action**:
    - Identify the *Source of Truth* (e.g., "Project Requirements", "Architecture Diagram", "Approved Plan").
    - Identify the *Target* (e.g., "src/ directory", "Deployed API", "Database Schema").
    - Determine scope (e.g., "Authentication module only").

## 2. State Retrieval
- **Goal**: Gather facts about Current State.
- **Action**:
    - Use `list_dir` to map structure.
    - Use `find_symbol` to check for specific classes/functions.
    - Use `grep_search` to find usage patterns.
    - **CRITICAL**: Do not rely on memory or assumptions. Execute tools.

## 3. Gap Detection
- **Goal**: List discrepancies.
- **Heuristics**:
    - **Missing Files**: Expected file structure vs actual.
    - **Missing Logic**: Empty functions, `todo!()`, or missing error handling.
    - **Configuration Drift**: Env vars or constants different from defaults.

## 4. Reporting
- **Goal**: Present findings.
- **Format**:
    ```markdown
    ## Gap Analysis Report
    | Gap ID | Category | Description                                    | Severity | Remediation       |
    | ------ | -------- | ---------------------------------------------- | -------- | ----------------- |
    | G-01   | Missing  | `AuthService` struct is missing `login` method | High     | Implement `login` |
    ```

## 5. Verification
- **Goal**: Ensure analysis is accurate.
- **Action**:
    - Double-check "Missing" items to ensure they aren't named differently.
    - Verify "Extra" items aren't just utilities.

