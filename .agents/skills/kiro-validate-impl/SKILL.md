---
name: kiro-validate-impl
description: Lean contract for kiro-validate-impl: validate feature-level implementation integration.
---

# kiro-validate-impl

Inputs
- TASKS_MD
- TEST_RESULTS
- DESIGN_MD
- REQUIREMENTS_MD

Outputs
- Core Indicators: VALIDATION_RESULT, ISSUES, VERIFICATION_EVIDENCE

Boundaries
- Validation only; no new feature work.

Rules
- Read tasks.md produced by kiro-spec-tasks
- Read test results; read design.md and requirements.md
- Runnable artifacts (binary, TUI, service): run the real thing against real data in its default configuration first, then one narrow/alternate size, and capture its output (terminal capture with escape codes, HTTP response, log) — a green test suite alone is not VERIFICATION_EVIDENCE
- NO-GO findings are appended to tasks.md as rework tasks in bugfix order (failing reproduction test → fix → pass), each traced to requirement IDs; re-validate after they close
- Output VALIDATION_RESULT and VERIFICATION_EVIDENCE
