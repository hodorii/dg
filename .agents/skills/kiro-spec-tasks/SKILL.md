---
name: kiro-spec-tasks
description: Lean contract for kiro-spec-tasks: generate implementation tasks from a spec.
---

# kiro-spec-tasks

Inputs
- REQUIREMENTS_MD
- DESIGN_MD

Outputs
- Core Indicators: TASKS_MD, TASK_LIST

Boundaries
- Do not implement; only task decomposition.

Rules
- Read requirements.md produced by kiro-spec-requirements
- Read design.md produced by kiro-spec-design
- Read `rules/tasks-generation.md` from this skill's directory for task decomposition and ordering rules
- Read `rules/tasks-parallel-analysis.md` from this skill's directory to mark `(P)` parallel tasks and `_Depends:_` ordering
- Read `{{TEMPLATES}}/specs/tasks.md` for document structure; produce tasks.md with explicit task entries
- If `bugfix.md` exists, follow the Bugfix Specs section of `rules/tasks-generation.md`
- Do not modify code or run tests
