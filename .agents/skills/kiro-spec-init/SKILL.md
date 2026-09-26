---
name: kiro-spec-init
description: Lean contract for kiro-spec-init: initialize a new specification with minimal artifacts.
---

# kiro-spec-init

Inputs
- brief.md (optional)
- roadmap.md (optional)

Outputs
- Core Indicators: SPEC_JSON

Boundaries
- Do not generate requirements/design/tasks.

Rules
- Read brief.md (if present)
- Read `{{STEERING}}/roadmap.md` (inclusion: manual) if present
- Generate spec.json from `{{TEMPLATES}}/specs/init.json`; requirements.md is produced by kiro-spec-requirements
- Do not create other spec artifacts
