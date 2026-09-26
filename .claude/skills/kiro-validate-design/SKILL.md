---
name: kiro-validate-design
description: Lean contract for kiro-validate-design: validate the technical design against requirements.
---

# kiro-validate-design

Inputs
- DESIGN_MD
- REQUIREMENTS_MD

Outputs
- Core Indicators: DESIGN_VALID, BOUNDARY_COMPLIANCE, ISSUES

Boundaries
- Design validation only; no implementation changes.

Rules
- Read design.md produced by kiro-spec-design
- Read requirements.md produced by kiro-spec-requirements
- Read `rules/design-go-nogo.md` from this skill's directory for review scope and GO/NO-GO criteria
- Output DESIGN_VALID or ISSUES
