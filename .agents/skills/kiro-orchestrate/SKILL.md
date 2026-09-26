---
name: kiro-orchestrate
description: Lean contract for kiro-orchestrate: glue orchestration of kiro steps.
---

# kiro-orchestrate

Inputs
- ticket.md
- brief.md

Outputs
- Core Indicators: TICKET_INGESTED, FLOW_STEPS, NEXT_COMMAND

Boundaries
- Do not execute steps beyond orchestration scope.

Rules
- Read ticket.md produced by external tracker
- Read brief.md produced by kiro-discovery
- Read `{{STEERING}}/ticket-workflow.md` (inclusion: manual) for ticket ingestion policy and phase gates
- Define NEXT_COMMAND after each stage
- Do not auto-run downstream steps
