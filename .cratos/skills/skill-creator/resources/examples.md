# Skill Creation Examples

## Example 1: Simple Utility Skill

**User Request**: "Create a skill to quickly format JSON files."

**Generated SKILL.md**:
```markdown
---
name: json-formatter
description: A utility skill to valid and format JSON files.
version: 1.0.0
triggers:
  - "format json"
  - "json create"
---

# JSON Formatter

## Role
- Validates JSON syntax.
- Formats JSON with 2-space indentation.

## Core Rules
1. Always use `jq` or built-in python tools for formatting.
2. Never overwrite the file without successful validation first.
3. Backup the original file before modification if > 1MB.

## Workflow
1. Check file existence.
2. Validate syntax.
3. Format and save.
```

## Example 2: Complex Workflow Skill

**User Request**: "I need a deployment agent for Kubernetes."

**Generated SKILL.md**:
```markdown
---
name: k8s-deployer
description: Handles deployment of services to Kubernetes clusters.
triggers:
  - "deploy to k8s"
  - "kubectl apply"
---

# K8s Deployer

## Role
- templating feature manifests.
- applying manifests to specific namespaces.
- verifying deployment status.

## Core Rules
1. NEVER deploy to `prod` without explicit user confirmation (y/n).
2. Always dry-run (`--dry-run=client`) before applying.
3. Rollback immediately if health checks fail within 2 mins.

## Resources
- **Protocol**: `resources/deployment-steps.md`
- **Manifest Templates**: `resources/templates/`
```
