# Task Generation Rules

## Core Principles

### 1. Natural Language Descriptions
Focus on capabilities and outcomes, not code structure.

**Describe**:
- What functionality to achieve
- Business logic and behavior
- Features and capabilities
- Domain language and concepts
- Data relationships and workflows

**Avoid**:
- File paths and directory structure
- Function/method names and signatures
- Type definitions and interfaces
- Class names and API contracts
- Specific data structures

**Rationale**: Implementation details (files, methods, types) are defined in design.md. Tasks describe the functional work to be done.

### 2. Task Ordering Principle

**Order implies dependency**: Task N implicitly depends on all tasks before it. This is the primary dependency mechanism.

**Tasks must follow this phase order**:
1. **Foundation**: Environment setup, test infrastructure, shared utilities, database schema, configuration
2. **Core**: Primary feature implementation (parallel-capable tasks grouped here)
3. **Integration**: Wiring components together, cross-boundary connections
4. **Validation**: E2E tests, edge cases, regression checks

**Rationale**: Foundation work unblocks everything else. Placing setup tasks early prevents downstream blocking. Core tasks can often run in parallel because foundation is already complete.

### 3. Task Integration & Progression

**Every task must**:
- Build on previous outputs (no orphaned code)
- Connect to the overall system (no hanging features)
- Progress incrementally (no big jumps in complexity)
- Respect architecture boundaries defined in design.md (Architecture Pattern & Boundary Map)
- Honor interface contracts documented in design.md
- Use major task summaries sparingly—omit detail bullets if the work is fully captured by child tasks.

**End with integration tasks** to wire everything together.

### 4. Dependency Declaration

**Default**: Sequential ordering handles most dependencies (task N depends on tasks before it).

**Explicit declaration required when**:
- A task depends on a specific task in a different major-task group (cross-boundary)
- The dependency is non-obvious from ordering alone
- A task can skip ahead of its position (declared via `(P)`) but still needs specific prior work

**Format**: `_Depends: 1.2, 2.3_` — placed alongside `_Requirements:_` in task detail sections.

**Do not over-annotate**: If a task simply depends on the task directly before it, ordering alone is sufficient.

### 5. Boundary Scope

**Each task should declare its component boundary** using design.md component/module names:
- `_Boundary: AuthService_` or `_Boundary: API Layer, UserRepository_`
- Helps validate parallel safety: tasks with non-overlapping boundaries are parallel candidates
- Helps agents understand scope: what to touch and what not to touch

**When to use**: Required for tasks marked `(P)` to validate parallel safety. Omit for sequential tasks where scope is obvious from the description.

**Boundary rule**:
- Each executable task should stay within a single responsibility boundary
- If work must cross boundaries, make it an explicit integration task rather than a normal implementation task
- Do not hide cross-boundary coordination inside a task that appears local

### 6. Flexible Task Sizing

**Guidelines**:
- **Major tasks**: As many sub-tasks as logically needed (group by cohesion)
- **Sub-tasks**: 1-3 hours each, 3-10 details per sub-task
- Balance between too granular and too broad

**Don't force arbitrary numbers** - let logical grouping determine structure.

### 7. Requirements Mapping

**End each task detail section with**:
- `_Requirements: X.X, Y.Y_` listing only requirement IDs (comma-separated). No descriptive text, parentheses, or labels such as "all".
- For cross-cutting requirements, list every relevant requirement ID. All requirements MUST have numeric IDs in requirements.md. If an ID is missing, stop and correct requirements.md before generating tasks.
- Reference components/interfaces from design.md when helpful (e.g., `_Contracts: AuthService API`)

### 7.5 Observable Completion

**Each executable task must include at least one detail bullet that describes the observable completed state**:
- Phrase it as a deliverable, runtime behavior, persisted state, UI state, endpoint behavior, test result, or integration outcome
- Avoid vague bullets like "implement support", "wire things up", or "handle logic" unless paired with a concrete observable result
- Prefer making one detail bullet clearly answer: "What will be true when this task is done?"
- Keep this within the existing task body; do not add extra bookkeeping fields
- **User-visible output (TUI/GUI/CLI)**: the DONE bullet names what the rendered frame or output shows — cells, styles, borders, connectors, exit code. Its test asserts the rendered buffer/output, not internal data structures; a passing structure-level test does not satisfy DONE.

### 7.6 Difficulty (implementer matching)
- Every executable sub-task carries `_Difficulty: low | mid | high_`.
  - `low`: one file, signatures and test cases given verbatim in the brief, no cross-module reading.
  - `mid`: composes 2–3 existing module APIs; needs reading, not design.
  - `high`: algorithmic (layout, parsing, state machines), cross-cutting integration, or UI wiring across panels.
- `kiro-impl` uses this field to pick the implementer; a `low` brief must be self-contained (files, signatures, tests, verify command).

### 7.7 BizProcess Traceability (when `biz-process.md` exists)
- If a task realizes a biz-process node, add `_BizProcess: <BP-ID>.<L-level>` (e.g., `_BizProcess: BP-SIGNUP.L4`) to its detail section.
- Add `_Verify:` only when the task's verification method is directly derived from that node's Logic(AST)/step flow (e.g., `_Verify: BP-SIGNUP.L2.logic`).
- E2E/Validation tasks (phase 4) must derive their scenarios from `biz-process.md` L1~L4 user flows, cross-checked against requirements, and reference the source node via `_BizProcess:`.
- The Coverage Review (§ Coverage Review) should report biz-process node coverage when `biz-process.md` is present — every L1~L3 node traced to at least one task.

### 8. Code-Only Focus

**Include ONLY**:
- Coding tasks (implementation)
- Testing tasks (unit, integration, E2E)
- Technical setup tasks (infrastructure, configuration)

**Exclude**:
- Deployment tasks
- Documentation tasks
- User testing
- Marketing/business activities

## Task Plan Review Gate

Before writing `tasks.md`, review the draft task plan and repair local issues until the plan passes or a true spec gap is discovered.

### Coverage Review

- Every requirement ID from `requirements.md` must appear in at least one task.
- Every design component, interface/contract, integration point, runtime prerequisite, and validation concern from `design.md` must be represented by at least one task.
- If coverage is missing because the task plan is incomplete, repair the draft tasks and review again.
- If coverage cannot be added cleanly because requirements or design are ambiguous, contradictory, or underspecified, stop and return to the requirements/design phase instead of papering over the gap in `tasks.md`.

### Executability Review

- Every sub-task must be executable as written, usually within 1-3 hours.
- Every sub-task must produce a verifiable deliverable (behavior, artifact, endpoint, UI state, config, migration, test, or integration result).
- Every executable sub-task must include at least one detail bullet that states the observable completion condition; for user-visible output it references the rendered frame/output (§7.5).
- Split tasks that combine multiple independently verifiable outcomes.
- Split tasks that combine multiple responsibility boundaries unless they are explicit integration tasks.
- If many tasks require broad `_Boundary:_` scopes or repeated cross-boundary coordination, stop and return to design or roadmap decomposition instead of forcing the spec through task generation.
- Merge or collapse tasks that are too small, bookkeeping-only, or not meaningful execution units.
- Make implicit prerequisites explicit as preceding tasks.
- Re-check `_Depends:_`, `_Boundary:_`, and `(P)` markers after edits so concurrency claims still match the design boundaries and dependency graph.

### Review Loop

- Run the review gate on the draft task plan before writing `tasks.md`.
- If issues are task-plan-local, repair the draft and re-run the review gate.
- Keep the loop bounded: no more than 2 review-and-repair passes before escalating a real spec gap.
- Write `tasks.md` only after the review gate passes.

### Optional Test Coverage Tasks

- When the design already guarantees functional coverage and rapid MVP delivery is prioritized, mark purely test-oriented follow-up work (e.g., baseline rendering/unit tests) as **optional** using the `- [ ]*` checkbox form.
- Only apply the optional marker when the sub-task directly references acceptance criteria from requirements.md in its detail bullets.
- Never mark implementation work or integration-critical verification as optional—reserve `*` for auxiliary/deferrable test coverage that can be revisited post-MVP.

## Task Hierarchy Rules

### Maximum 2 Levels
- **Level 1**: Major tasks (1, 2, 3, 4...)
- **Level 2**: Sub-tasks (1.1, 1.2, 2.1, 2.2...)
- **No deeper nesting** (no 1.1.1)
- If a major task would contain only a single actionable item, collapse the structure and promote the sub-task to the major level (e.g., replace `1.1` with `1.`).
- When a major task exists purely as a container, keep the checkbox description concise and avoid duplicating detailed bullets—reserve specifics for its sub-tasks.

### Sequential Numbering
- Major tasks MUST increment: 1, 2, 3, 4, 5...
- Sub-tasks reset per major task: 1.1, 1.2, then 2.1, 2.2...
- Never repeat major task numbers

### Format
- Checkbox layout, markers, and detail fields: follow `{{TEMPLATES}}/specs/tasks.md`.
- `(P)` marking: `tasks-parallel-analysis.md`.

### Bugfix Specs (when `bugfix.md` exists)
- Fixed order: reproduction test (must fail before the fix) → unchanged-behavior tests (must pass before the fix) → the fix → both suites pass.
- `_Requirements:` references bugfix.md IDs (`1.x` 결함, `2.x` 기대, `3.x` 불변); every `2.x` and `3.x` maps to a test task.
