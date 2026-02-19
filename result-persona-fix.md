# Final Report: Persona System Prompt Bugs Fix

## Summary
Fixed two critical bugs affecting Cratos identity and persona loading.

## Changes
- **Task PERSONA-001**: Implemented `load_persona_names` in `crates/cratos-tools/src/builtins/config.rs` to dynamically load available personas from TOML files.
- **Task LLM-001/002**: Updated `convert_messages` in `cratos-llm` (Gemini/Anthropic) to concatenate all system instructions instead of overwriting them.

## Verification
- **Clippy**: All warnings resolved (including `unnecessary_map_or`).
- **Tests**: `cratos-tools` and `cratos-llm` tests pass.
- **Behavior**: System instructions now preserve Cratos identity + user instructions.

## Status
READY TO MERGE
