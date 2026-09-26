---
name: kiro-spec-quick
description: Lean contract for kiro-spec-quick: run init → requirements → design → tasks in one pass, interactive or --auto.
---

# kiro-spec-quick

Inputs
- $ARGUMENTS: feature description [--auto]
- brief.md (optional, from kiro-discovery)

Outputs
- Core Indicators: SPEC_JSON, REQUIREMENTS_MD, DESIGN_MD, TASKS_MD, SANITY_REVIEW

Boundaries
- Skips kiro-validate-gap / kiro-validate-design; each skill's self-check still runs.
- Does not implement.

Rules
- Mode: `--auto` → all phases without prompts; otherwise prompt between phases (decline → stop and show state).
- Phase 1 (init): feature name = kebab-case of the description (2–4 words); reuse a `{{SPECS}}/<name>/` that holds only brief.md, else suffix `-2`, `-3` on collision. Write spec.json from `{{TEMPLATES}}/specs/init.json` (`{{FEATURE_NAME}}`, `{{TIMESTAMP}}` = `date -u +%Y-%m-%dT%H:%M:%SZ`, `language` from the user's input language). Use brief.md as the description when present.
- Phase 2–4: delegate to `$kiro-spec-requirements`, `$kiro-spec-design -y`, `$kiro-spec-tasks -y`; ignore their standalone "Next Step" output; print `Phase N/4 complete`.
- `$kiro-biz-process` is opt-in after Phase 2; design consumes biz-process.md when present.
- Sanity review after Phase 4 (fresh sub-agent when available, pass file paths only): coherence across the three documents, missing prerequisites or coverage, plausibility of `(P)` / `_Depends:_` / `_Boundary:_`. Task-local issues → fix tasks.md once and re-review; requirements/design gaps → stop and report, do not claim implementation-ready.
- Any failure: stop, show completed phases, suggest `$kiro-spec-<next-phase> {feature}`.
- Final output in spec.json language: generated files, skipped validations, sanity verdict, next step `$kiro-impl {feature}`.
