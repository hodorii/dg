---
name: kiro-steering-custom
description: Lean contract for kiro-steering-custom: project-specific steering guidance.
---

# kiro-steering-custom

Inputs
- brief.md
- steering_notes.md

Outputs
- Core Indicators: CUSTOM_GUIDANCE, BOUNDARY_TWEAKS

Boundaries
- Steering customization only; no policy changes outside scope

Rules
- Read brief.md produced by kiro-steering
- Read steering_notes.md produced by kiro-steering-custom
- Read `../kiro-steering/rules/steering-principles.md` (shared with kiro-steering) for content granularity and lean-maintenance rules
- Read `{{TEMPLATES}}/steering-custom/` for custom steering file structure
- Emit CUSTOM_GUIDANCE for project context
