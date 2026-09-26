---
name: kiro-spec-status
description: Lean contract for kiro-spec-status: report progress and gate status.
---

# kiro-spec-status

Inputs
- spec.json
- milestones.md (optional)

Outputs
- Core Indicators: STATUS_REPORT, PROGRESS, NEXT_GATES

Boundaries
- Report status only; do not modify specs.

Rules
- Read spec.json produced by kiro-spec-init
- `phase` values (SSoT, in order): `initialized` → `requirements-generated` (or `bugfix-generated`) → `biz-process-generated` → `design-generated` → `tasks-generated` → `implementation` → `completed`; `approvals.<doc>.{generated,approved}` per document
- Read milestones.md if present
- Output concise status and blockers
