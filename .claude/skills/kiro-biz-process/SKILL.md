---
name: kiro-biz-process
description: Lean contract for kiro-biz-process: user-perspective L1~L6 process unfold anchored to value-chain Units.
---

# kiro-biz-process

Inputs
- requirements.md (approved)
- {{STEERING}}/value-chain.md (SSoT, read-only)
- spec.json

Outputs
- Core Indicators: BIZ_PROCESS_MD, VALUE_CHAIN_LINKS

Boundaries
- User viewpoint only; no verification criteria (V-model right arm lives elsewhere).
- Never edit value-chain.md or steering.

Rules
- Read `rules/biz-process-rules.md` from this skill's directory for viewpoint, BPMN, value-chain linkage, drill-down levels, approval gates, and the value-chain bootstrap guide
- Read `{{TEMPLATES}}/specs/biz-process.md` for document structure
- Stop if requirements.md is unapproved or value-chain.md is missing (present the bootstrap guide; do not create it)
- Each matched Unit → one L1 Process with `valueChainRef`; no match → mark `⚠ 가치사슬 매핑 필요` and continue after user ack
- Gate each L1 with the user (`-y`: single gate after full drill-down); on rejection revise that level only
- Write `{{SPECS}}/{feature}/biz-process.md`; update spec.json (`phase`, `approvals.bizProcess`, `updated_at`)
- Next: `$kiro-spec-design {feature}`
