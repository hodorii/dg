---
name: multi-agent-sessions
description: Lean contract for multi-agent-sessions — during multi-agent collaboration on a project, check what any agent (Claude Code, opencode, agy, Kiro CLI) did, by reading each agent's own on-disk session storage directly.
---

# multi-agent-sessions

Inputs
- source (optional; default: `claude`; one of `claude` | `opencode` | `agy` | `kiro`)
- project path (optional; default: cwd)
- session id or prefix (optional; default: most recent session)
- keyword (optional; for search across sessions)

Outputs
- Core Indicators: SESSION_LIST, SESSION_VIEW, SEARCH_HITS

Boundaries
- Read-only: never modify or delete any agent's session storage.
- Read each agent's own storage directly (file or local db) — never by launching that agent's CLI.
- Local inspection only; never transmit session content externally without explicit approval.

Rules
- Never hardcode a specific user or project path — every storage root is derived from `$HOME`/XDG env vars, and every project directory is derived from the target project's absolute path.
- Per source, where the data lives and how solid the read is:
  - `claude`: `~/.claude/projects/<absolute-project-path with "/" replaced by "-">/<uuid>.jsonl`, one JSON object per line. `type` is `user`/`assistant`/...; `message.content` is a string or blocks (`text`, `thinking`, `tool_use`, `tool_result`). Fully verified.
  - `opencode`: sqlite db at `$XDG_DATA_HOME/opencode/opencode.db` (default `~/.local/share/opencode/opencode.db`), opened read-only. `session.directory` = project path; `message.data` (JSON) is that message's info; `part.data` (JSON) is one content part (`text`, `reasoning`, `tool`, `step-start`/`step-finish`). Fully verified — do not shell out to the `opencode` CLI for this (its own `export`/`session list` piped stdout was observed truncating at 64KiB).
  - `agy` (Antigravity CLI): conversation index from `~/.gemini/antigravity-cli/conversation_summaries.db` (sqlite, always current) when present, else the `cache/conversation_metadata.json` snapshot (plain JSON, keyed by conversation id, filter by `summary.WorkspaceURIs`) — that snapshot can lag a just-created session, which the sqlite path exists to fix. Both verified. Turn content is read from `~/.gemini/antigravity-cli/brain/<id>/.system_generated/logs/transcript.jsonl` — a clean per-step JSON log (`source`: `USER_EXPLICIT`/`MODEL`/`SYSTEM` → role; `content`/`thinking`/`tool_calls` per step), verified against a real session. Falls back to heuristically extracting printable UTF-8 runs out of `~/.gemini/antigravity-cli/conversations/<id>.db`'s protobuf `steps.step_payload` blobs only when a conversation has no `transcript.jsonl` — that fallback is best-effort and noisy.
  - `kiro` (Kiro CLI): `~/.kiro/sessions/cli/<uuid>.json` sidecar carries `cwd` (project filter), `title`, `created_at`/`updated_at`; the matching `<uuid>.jsonl` is a clean per-turn log, one JSON object per line — `kind: "Prompt"` (user) / `"AssistantMessage"` (assistant), `data.content` is a list of `{kind: "text"|"thinking", data}` items. Fully verified against a real populated session. (A separate `~/.kiro/sessions/<hash>/sess_*/session.json` mechanism also exists but every example found had zero turns — not used here.)
- List sessions: newest first, labeled by title/first-user-text.
- Read a session: stream turns in order, render each by its own type; `--full` disables truncation, `--thinking` includes thinking/reasoning blocks (claude/opencode only).
- Search across sessions: per-project by default, `--all-projects` sweeps every project each source knows about.
- Timeline: `timeline --source all` merges every source's session list for one project into a single chronological, titles-only feed. Sources mix timestamp representations (claude/opencode: local time; agy/kiro: UTC `Z` strings) — always compare/sort by the POSIX epoch each converts to, never by the formatted string, or the feed misorders by this machine's UTC offset.
- Use `scripts/session.py {list|show|search|timeline} --source {claude|opencode|agy|kiro|all}` (`all` only for `timeline`) for all of the above instead of re-deriving the parsing logic by hand.
