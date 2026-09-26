# Agentic SDLC — Kiro-style Spec-Driven Development

## Paths
Skills refer to these names; relocate by editing this table.

| Name | Default | Holds |
|---|---|---|
| `{{SPECS}}` | `.kiro/specs` | one directory per feature or fix: spec.json, requirements.md or bugfix.md, biz-process.md, design.md, tasks.md, research.md |
| `{{STEERING}}` | `.kiro/steering` | project memory — rules and judgment |
| `{{REFERENCE}}` | `.kiro/reference` | regenerable measurements — one command rebuilds it; lazy-loaded; header rule `{{REFERENCE}}/reference-convention.md` |
| `{{TEMPLATES}}` | `.kiro/settings/templates` | artifact structure only: `specs/`, `steering/`, `steering-custom/` |
| `{{SKILLS}}` | `.agents/skills` | `kiro-*/`: `SKILL.md` contract (Inputs / Outputs / Boundaries / Rules), `rules/` method, `templates/` sub-agent prompts; `.claude/skills` links here |

- `.kiro/specs` and `.kiro/steering` are Kiro IDE conventions.
- The methodology lives in `methodology/`; `{{SKILLS}}`/`{{TEMPLATES}}` are symlinked from there — `methodology/README.md`.
- The project root `AGENTS.md` is a pointer file (not a symlink) to this file — protects this file from tools that overwrite `AGENTS.md` in place.
- A local `AGENTS.md` in a subdirectory carries folder-specific context.

## Principles
이해 > 문서. 산출물은 "이해했음을 증명하는 최소 증거"다.
- **SSoT**: 규칙·값·템플릿·경로는 한 곳에만, 나머지는 참조. `value-chain.md`는 product owner 소유 — 스킬은 읽기만.
- **SRP**: 파일 하나 = 책임 하나.
- **Descriptable Name**: 이름이 내용을 말한다.
- **지침 / 히스토리 분리**: `{{SKILLS}}`·`{{STEERING}}`에는 앞으로의 규칙만. 경위·결정·검증 기록은 `{{SPECS}}`와 git.
- **MoE**: 각 스킬은 독립 전문가 — 자기 rules·자기 산출물만, 필요한 단계에서만 로드. 산출물에 지시를 싣지 않는다; 평가 기준은 SKILL.md `Core Indicators`.
- **Lean**: 지침·산출물에 적용 — 개조식·명사형 동사, 부연 금지, 예시 1개. 인용구(`>`)는 보조 설명. 구조 장치(번호·섹션·표)는 정보를 담을 때만. 사용자 대면 보고는 반대로 결과 먼저·완전한 문장, 화살표 체인·약어 금지.

## Artifacts
- `## 정의` opens every spec artifact: "[A]를 위해 [B]를 하는 [C]이다".
- Acceptance criteria `N.M: [condition] → [result]`; IDs unprefixed as numbered in requirements.md; contiguous ranges `2.1~2.5` allowed.
- Traceability: value-chain Unit ↔ biz-process L1 (`valueChainRef` / `bizProcessRef`, 값 복제 금지) → requirement IDs on L2/L3 → design Boundary Commitments → tasks `_Requirements:` / `_Boundary:` / `_BizProcess:` → validate-impl coverage.
- Boundary term: discovery `Boundary Candidates` → requirements boundary context → design `Boundary Commitments` → tasks `_Boundary:`.
- Written in `spec.json.language`; ~200 lines cap, over = split signal.

## Workflow
- Phase 0 (optional): `$kiro-steering`, `$kiro-steering-custom`
- Discovery: `$kiro-discovery "idea"`
- Phase 1 (Specification): `$kiro-spec-quick {feature} [--auto]` or step by step:
  - `$kiro-spec-init {feature}`
  - `$kiro-spec-requirements {feature}`
  - `$kiro-validate-gap {feature}` (optional)
  - `$kiro-biz-process {feature}`
  - `$kiro-spec-design {feature} [-y]`
  - `$kiro-validate-design {feature}` (optional)
  - `$kiro-spec-tasks {feature} [-y]`
  - Multi-spec: `$kiro-spec-batch`
  - Bugfix: `$kiro-bugfix {fix}` → `$kiro-spec-design` → `$kiro-spec-tasks`
- Phase 2 (Implementation): `$kiro-impl {feature} [tasks]` → `$kiro-validate-impl {feature}` → `$kiro-verify-completion`
- Ticket-driven: `$kiro-orchestrate`
- Progress: `$kiro-spec-status {feature}`

## Skills
- Invoke with `$kiro-<name>`; `/skills` lists them with their contracts' descriptions.
- If there is even a 1% chance a skill applies, invoke it.

## Rules
- Approval gates: Requirements or Bugfix → BizProcess → Design → Tasks → Implementation. Human review at each gate; `-y` only for an intentional fast-track.
- Reason in English. Follow the user's instructions precisely; within that scope act autonomously end-to-end, asking only when essential information is missing.
- When the user describes a problem or thinks aloud, the deliverable is an assessment: report and stop; change things only when asked.
- Report only what a tool result in this session evidences; say plainly what is unverified. Pause for the user only for destructive or irreversible actions, real scope changes, or input only they can give.

## Steering
- `inclusion` front matter (Kiro): `always` (default when absent) every session; `manual` only when a skill reads it by path; `fileMatch` / `auto` per Kiro docs.
- `always` baseline: `product.md`, `tech.md`, `structure.md`. Custom files via `$kiro-steering-custom`; every new file states its `inclusion`.
- Steering Sync: on code diff, additive, per repository, at ticket or feature completion.
- 승격(elevation): a standard-worthy artifact moves to `{{STEERING}}` or `{{REFERENCE}}`.
