---
name: kiro-spec-design
description: Lean contract for kiro-spec-design: generate technical design per spec.
---

# kiro-spec-design

Inputs
- REQUIREMENTS_MD
- BRIEFS (from brief.md)

Outputs
- Core Indicators: DESIGN_MD, DESIGN_BOUNDARIES, INTERFACES

Boundaries
- Design only; no implementation changes.

Rules
- Read requirements.md produced by kiro-spec-requirements
- Read brief and `{{TEMPLATES}}/specs/design.md` for document structure; produce design.md with boundary commitments
- Read `rules/design-discovery-light.md` (extension of existing system) or `rules/design-discovery-full.md` (new feature) from this skill's directory before drafting
- Read `rules/design-synthesis.md` from this skill's directory for generalization / build-vs-adopt / simplification before writing
- Write research.md from `{{TEMPLATES}}/specs/research.md` when discovery yields alternatives or decisions that belong outside design.md
- Read `rules/design-principles.md` from this skill's directory for structure, Components & Interfaces, error handling, and testing strategy requirements
- Read `rules/verification-mapping.md` from this skill's directory for the L1~L6 test-level mapping and verification Depth before writing Testing Strategy
- Read `rules/design-self-check.md` from this skill's directory before finalizing to verify requirements coverage
- If `bugfix.md` exists instead of requirements.md, read `rules/design-bugfix.md` from this skill's directory and follow it instead of the discovery/synthesis rules
- Do not generate tasks or code
