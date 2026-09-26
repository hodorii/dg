# Design Authoring Rules

## Principles
- **Boundary first**: state what this spec owns / does not own / may depend on before any component. An ambiguous responsibility seam means the design is not ready.
- **Dependency direction**: declare the layer order once (e.g., `types → config → repo → service → ui`); imports go leftward only. Implementation and review treat violations as errors.
- **What, not how**: interfaces, contracts, state transitions, failure modes — not algorithms.
- **No speculative abstraction**: nothing that exists only for hypothetical future scope.
- **Self-contained**: design.md restates every decision it depends on; research.md is background only.

## Section Rules (order = `{{TEMPLATES}}/specs/design.md`)
- **Boundary Commitments**: In-Scope / Out-of-Scope / Allowed Dependencies (external libs + version, internal direction) / Revalidation Triggers — all four non-empty.
- **Architecture**: Boundary Map (Mermaid) when 3+ components interact. Technology Stack only for layers this feature touches (tool + version + role). Key Decisions as `결정 — 이유` one-liners; alternatives in research.md.
- **System Flows**: Mermaid sequence/state for non-obvious flows only; omit the section otherwise. Tag decisions with requirement IDs.
- **Components & Interfaces**: one block per component — Intent, Requirements (IDs), public signatures in the implementation language including error types. Dependencies table (Inbound/Outbound/External, P0/P1/P2) only for external integrations or cross-boundary contracts. Presentational/UI components: summary bullets only.
- **Data Models**: domain types, persistence, invariants; if the interface blocks already cover it, say so.
- **Error Handling / Testing Strategy**: feature-specific decisions only, tagged with requirement IDs; level mapping and Depth from `rules/verification-mapping.md`; baseline practices live in steering.
- **File Structure Plan**: every component has a file path; paths must not imply ownership beyond In-Scope.

## Mermaid
- Plain Mermaid, no styling. Node IDs alphanumeric/underscore; labels without `()[]"/`. Subgraphs sparingly.

## Deduplication
- Text next to a diagram carries only decisions/trade-offs not visible in the diagram.
- A requirement↔component mapping stated in the component block is not repeated elsewhere.
