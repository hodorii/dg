# agentic-psdd

Agentic PSDD (Progressive Spec-Driven Development) — Kiro-style spec workflow for an agentic SDLC: `AGENTS.md` (router + principles), `skills/kiro-*` (lean skill contracts), `templates/` (artifact structure).
Project-specific knowledge never lives here — it belongs to the consuming project's `.kiro/steering/`, `.kiro/specs/`, `.kiro/reference/`.

## Layout in a consuming project
```
methodology/                      this repo (submodule, subtree, or copy)
AGENTS.md              pointer file, not a symlink -> "see methodology/AGENTS.md"
.agents/skills         -> ../methodology/skills
.claude/skills         -> ../methodology/skills
.kiro/settings/templates -> ../../methodology/templates
```
Skills and templates are symlinked; `AGENTS.md` is deliberately not — many agent CLIs treat a project's root `AGENTS.md` as their own memory file and write to it in place, which through a symlink would corrupt the source `methodology/AGENTS.md` instead of just the pointer. Paths used by skills are names (`{{SPECS}}`, `{{STEERING}}`, `{{REFERENCE}}`, `{{TEMPLATES}}`, `{{SKILLS}}`) resolved by the `## Paths` table in `methodology/AGENTS.md`.

## Install
- Submodule: `git submodule add <url> methodology && methodology/install.sh link`
- Subtree: `git subtree add --prefix=methodology <url> main --squash && methodology/install.sh link`
- Copy (no dependency): `git clone <url> /tmp/m && /tmp/m/install.sh copy /path/to/project`

`install.sh link` replaces the skills/templates locations with symlinks and (re)writes the root `AGENTS.md` pointer; `copy` writes real files everywhere, including the full content into `AGENTS.md`, and can be re-run to update.
