---
name: kiro-review
description: Lean contract for kiro-review: adversarial task-local review results.
---

# kiro-review

Inputs
- TASK_ID
- TASK_TEXT
- REQUIREMENTS_MD
- DESIGN_MD
- TASKS_MD
- git_diff

Outputs
- Core Indicators: VERDICT, REASONS, REMEDIATION

Boundaries
- Review only; no code changes.

Rules
- Read TASKS_MD produced by kiro-spec-tasks
- Read DESIGN_MD and REQUIREMENTS_MD
- Produce VERDICT: APPROVED or REJECTED and REMEDIATION if rejected
