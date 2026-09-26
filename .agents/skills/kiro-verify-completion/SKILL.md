---
name: kiro-verify-completion
description: Lean contract for kiro-verify-completion: verify feature completion with fresh evidence.
---

# kiro-verify-completion

Inputs
- spec_state
- final_validation_result

Outputs
- Core Indicators: DONE_EVIDENCE, VERIFICATION_REPORT

Boundaries
- Verification only; no new development.

Rules
- Read current state produced by kiro-validate-impl
- Produce VERIFICATION_REPORT and DONE_EVIDENCE
- Record lessons in the harness memory outside the repository (one note per lesson, one-line summary first; corrections and confirmed approaches with why they mattered; nothing the repo or git already records). A lesson that proves stable is promoted to `{{STEERING}}` as a rule
