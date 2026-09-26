---
name: kiro-discovery
description: Lean contract for kiro-discovery: determine path and decompose work into specs.
---

# kiro-discovery

Inputs
- brief_seed.md
- project_state.json (optional)

Outputs
- Core Indicators: PATH_DETECTED, BOUNDARIES_DEFINED, SPEC_ROUTES

Boundaries
- Determine path without enacting changes outside the discovery phase.

Rules
- Read brief_seed.md produced by kiro-orchestrate (or provided by user)
- Infer PATH_DETECTED from metadata
- Define SPEC_ROUTES and relevant boundaries
- Write brief.md / roadmap.md from `templates/brief.md` / `templates/roadmap.md` in this skill's directory
- Stop after outputting path and next-step guidance
