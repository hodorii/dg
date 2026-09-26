---
name: kiro-spec-requirements
description: Lean contract for kiro-spec-requirements: capture acceptance criteria and constraints.
---

# kiro-spec-requirements

Inputs
- brief.md

Outputs
- Core Indicators: REQUIREMENTS_MD, ACCEPTANCE_CRITERIA, BOUNDARY_FLAGS

Boundaries
- Do not include design or task details.

Rules
- Read brief.md produced by kiro-spec-init or discovery
- Read `rules/acceptance-criteria-format.md` from this skill's directory for acceptance-criteria syntax
- Read `{{TEMPLATES}}/specs/requirements.md` for document structure; produce requirements.md with explicit acceptance criteria
- Read `rules/requirements-self-check.md` from this skill's directory before finalizing to check boundary continuity
- Do not extend beyond defined boundary
