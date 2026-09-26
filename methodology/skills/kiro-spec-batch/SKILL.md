---
name: kiro-spec-batch
description: Lean contract for kiro-spec-batch: generate multiple specs from roadmap in parallel.
---

# kiro-spec-batch

Inputs
- roadmap.md
- brief.md (optional)

Outputs
- Core Indicators: SPEC_LIST, BRIEFS, DEPENDENCY_ORDER

Boundaries
- Do not generate design or tasks; only spec.json and requirements.md as output.

Rules
- Read `{{STEERING}}/roadmap.md` (inclusion: manual) produced by kiro-discovery or kiro-steering
- Read briefs for each new spec as needed
- Emit spec.json and requirements.md per new spec in dependency order
- Do not modify existing specs without explicit approval
