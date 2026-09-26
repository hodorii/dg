---
name: kiro-validate-gap
description: Lean contract for kiro-validate-gap: analyze implementation gap against requirements.
---

# kiro-validate-gap

Inputs
- REQUIREMENTS_MD
- DESIGN_MD

Outputs
- Core Indicators: GAP_ANALYSIS, RECOMMENDATIONS

Boundaries
- Gap analysis only; no code changes.

Rules
- Read requirements.md
- Read design.md
- Read `rules/gap-analysis.md` from this skill's directory for the analysis framework
- Output GAP_ANALYSIS and RECOMMENDATIONS
