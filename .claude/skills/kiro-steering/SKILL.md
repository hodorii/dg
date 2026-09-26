---
name: kiro-steering
description: Lean contract for kiro-steering: manage steering, value chain, and specs knowledge.
---

# kiro-steering

Inputs
- brief.md
- value-chain.md

Outputs
- Core Indicators: STEERING_STATE, VALUE_CHAIN_LINKS, BOUNDARY_COMMITMENTS

Boundaries
- Only manage steering knowledge; no code changes.

Rules
- Read brief.md produced by kiro-orchestrate
- Read value-chain.md produced by kiro-steering-custom or kiro-steering
- Read `rules/steering-principles.md` from this skill's directory for content granularity and lean-maintenance rules
- Read `{{TEMPLATES}}/steering/{product,tech,structure}.md` for baseline file structure
- Maintain boundary commitments in a separate artifact
- Do not modify specs directly
