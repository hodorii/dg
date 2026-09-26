---
name: kiro-debug
description: Lean binary contract for kiro-debug: execution-focused contract.
---

# kiro-debug

Inputs
- brief.md
- spec.json

Outputs
- Core Indicators: ROOT_CAUSE, CATEGORY, FIX_PLAN, VERIFICATION, NEXT_ACTION, CONFIDENCE, NOTES

Boundaries
- Scope limited to debugging within the current feature boundary; no code changes beyond the defined fix plan.

Rules
- Read brief.md produced by kiro-discovery
- Read spec.json produced by kiro-spec-init
- Read requirements.md produced by kiro-spec-requirements (if present)
- Read design.md produced by kiro-spec-design (if present)
- Read tasks.md produced by kiro-spec-tasks (if present)
- Dispatch a debugging sub-agent built from `../kiro-impl/templates/debugger-prompt.md` with blocker details
- Update Core Indicators fields accordingly
- NEXT_ACTION must be one of: RETRY_TASK | BLOCK_TASK | STOP_FOR_HUMAN
