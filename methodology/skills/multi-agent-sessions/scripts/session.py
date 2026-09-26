#!/usr/bin/env python3
"""Locate and read agent session transcripts for a project, straight from
each agent's own on-disk storage — never by launching the agent itself.

Sources (--source, default: claude):
  claude    Claude Code: ~/.claude/projects/<sanitized-path>/<uuid>.jsonl
  opencode  opencode: sqlite db at $XDG_DATA_HOME/opencode/opencode.db
            (session/message/part tables), opened read-only.
  agy       Antigravity CLI: ~/.gemini/antigravity-cli — conversation index
            is plain JSON, but each conversation's content is a sqlite db
            of protobuf blobs (no public schema). Content is recovered by
            heuristically pulling printable UTF-8 runs out of the blob, so
            `show`/`search` output for this source is best-effort and noisy.
  kiro      Kiro CLI: ~/.kiro/sessions/cli/<uuid>.json (+ matching .jsonl)
            — a clean per-turn log, fully verified against a real session.

Subcommands:
  list      List sessions for a project, newest first.
  show      Print one session's turns.
  search    Grep a keyword across sessions.
  timeline  Merge every source's sessions for a project into one
            chronological, titles-only timeline.

No path in this file is project- or user-specific: every project directory
is always derived from the target project's absolute path (never hardcoded),
and every agent's storage root is derived from $HOME / XDG env vars, never
from this machine's actual username.
"""
from __future__ import annotations

import argparse
import datetime
import glob
import json
import os
import re
import sqlite3
import sys

TRIM_LEN = 300
SQLITE_BUSY_TIMEOUT_MS = 5000


def _sqlite_ro_connect(path: str) -> sqlite3.Connection:
    """Open a sqlite db read-only, tolerating a concurrent writer.

    Without a busy_timeout, a db actively being written by its owning agent
    (e.g. agy writing conversation_summaries.db while a session is still
    live) can raise SQLITE_BUSY on the very first query. Callers that catch
    that as "no data here" then silently fall back to a stale snapshot
    instead of retrying — so wait out the lock here instead.
    """
    con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    con.execute(f"PRAGMA busy_timeout = {SQLITE_BUSY_TIMEOUT_MS}")
    return con


def _trim(s: str, full: bool) -> str:
    return s if full or len(s) <= TRIM_LEN else s[:TRIM_LEN] + "…"


def _iso(ms_or_s: float | None, ms: bool) -> str:
    if not ms_or_s:
        return ""
    value = ms_or_s / 1000 if ms else ms_or_s
    return datetime.datetime.fromtimestamp(value).isoformat(timespec="seconds")


def _epoch_from_iso_utc(s: str | None) -> float:
    """Parse an ISO-8601 UTC ('...Z') timestamp into a POSIX epoch (0 if unparsable).

    Needed because sources mix representations: claude/opencode timestamps
    are local-time already; agy/kiro emit UTC 'Z' strings. Comparing the
    formatted strings directly (as opposed to the epoch each converts to)
    would silently misorder a timeline by this machine's UTC offset.
    """
    if not s:
        return 0.0
    try:
        return datetime.datetime.fromisoformat(s.replace("Z", "+00:00")).timestamp()
    except ValueError:
        return 0.0


# ---------------------------------------------------------------------------
# claude: ~/.claude/projects/<sanitized-path>/<uuid>.jsonl
# ---------------------------------------------------------------------------


def claude_project_dir(project_path: str | None) -> str:
    abs_path = os.path.abspath(project_path or os.getcwd())
    sanitized = abs_path.replace(os.sep, "-")
    return os.path.expanduser(os.path.join("~", ".claude", "projects", sanitized))


def claude_session_files(pdir: str) -> list[str]:
    return sorted(glob.glob(os.path.join(pdir, "*.jsonl")), key=os.path.getmtime, reverse=True)


def claude_read_lines(path: str):
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError:
                continue


def _blocks_to_text(blocks) -> str:
    if not isinstance(blocks, list):
        return ""
    for block in blocks:
        if isinstance(block, dict) and block.get("type") == "text":
            return block.get("text", "")
    return ""


def claude_first_user_text(path: str, max_len: int = 80) -> str:
    for entry in claude_read_lines(path):
        if entry.get("type") == "user":
            content = entry.get("message", {}).get("content")
            text = content if isinstance(content, str) else _blocks_to_text(content)
            return " ".join(text.split())[:max_len]
    return "(no user message)"


def claude_render_block(block, full: bool, show_thinking: bool) -> str | None:
    if not isinstance(block, dict):
        return str(block)
    btype = block.get("type")
    if btype == "text":
        return _trim(block.get("text", ""), full)
    if btype == "thinking":
        if not show_thinking:
            return None
        return "[thinking] " + _trim(block.get("thinking", ""), full)
    if btype == "tool_use":
        return f"[tool_use] {block.get('name')}({_trim(json.dumps(block.get('input', {}), ensure_ascii=False), full)})"
    if btype == "tool_result":
        content = block.get("content")
        if isinstance(content, list):
            content = " ".join(b.get("text", "") for b in content if isinstance(b, dict) and b.get("type") == "text")
        return f"[tool_result] {_trim(str(content), full)}"
    return f"[{btype}]"


def claude_resolve_session_path(pdir: str, session_ref: str) -> str | None:
    files = claude_session_files(pdir)
    if not files:
        return None
    if session_ref in (None, "latest"):
        return files[0]
    for path in files:
        session_id = os.path.splitext(os.path.basename(path))[0]
        if session_id == session_ref or session_id.startswith(session_ref):
            return path
    return None


def claude_list_sessions(project_path: str | None, limit: int | None = None) -> list[dict]:
    pdir = claude_project_dir(project_path)
    out = []
    for path in claude_session_files(pdir):
        epoch = os.path.getmtime(path)
        out.append(
            {
                "id": os.path.splitext(os.path.basename(path))[0],
                "title": claude_first_user_text(path),
                "updated": datetime.datetime.fromtimestamp(epoch).isoformat(timespec="seconds"),
                "updated_epoch": epoch,
            }
        )
    return out[:limit] if limit else out


def claude_list(args) -> int:
    pdir = claude_project_dir(args.project)
    if not os.path.isdir(pdir):
        print(f"no session directory: {pdir}", file=sys.stderr)
        return 1
    for s in claude_list_sessions(args.project, args.limit):
        print(f"{s['id']}\t{s['updated']}\t{s['title']}")
    return 0


def claude_show(args) -> int:
    pdir = claude_project_dir(args.project)
    path = claude_resolve_session_path(pdir, args.session)
    if not path:
        print(f"session not found: {args.session!r} in {pdir}", file=sys.stderr)
        return 1
    for entry in claude_read_lines(path):
        etype = entry.get("type")
        if etype not in ("user", "assistant"):
            continue
        if args.role != "all" and etype != args.role:
            continue
        content = entry.get("message", {}).get("content")
        print(f"--- {etype} {entry.get('timestamp', '')} ---")
        if isinstance(content, str):
            print(_trim(content, args.full))
        else:
            for block in content or []:
                rendered = claude_render_block(block, args.full, args.thinking)
                if rendered is not None:
                    print(rendered)
    return 0


def claude_search(args) -> int:
    if args.all_projects:
        files = glob.glob(os.path.expanduser(os.path.join("~", ".claude", "projects", "*", "*.jsonl")))
    else:
        files = claude_session_files(claude_project_dir(args.project))
    hits = 0
    for path in files:
        for entry in claude_read_lines(path):
            if entry.get("type") not in ("user", "assistant"):
                continue
            content = entry.get("message", {}).get("content")
            text = content if isinstance(content, str) else _blocks_to_text(content) or json.dumps(content, ensure_ascii=False)
            if args.keyword.lower() in text.lower():
                session_id = os.path.splitext(os.path.basename(path))[0]
                print(f"{session_id}\t{entry.get('timestamp', '')}\t{' '.join(text.split())[:200]}")
                hits += 1
    if hits == 0:
        print("no matches", file=sys.stderr)
        return 1
    return 0


# ---------------------------------------------------------------------------
# opencode: sqlite db, read directly (session / message / part tables).
# message.data holds the same object opencode calls a message's "info";
# part.data holds one part object, both as JSON text columns.
# ---------------------------------------------------------------------------


def opencode_db_path() -> str:
    data_home = os.environ.get("XDG_DATA_HOME") or os.path.expanduser(os.path.join("~", ".local", "share"))
    return os.path.join(data_home, "opencode", "opencode.db")


def opencode_connect() -> sqlite3.Connection | None:
    path = opencode_db_path()
    if not os.path.isfile(path):
        return None
    return _sqlite_ro_connect(path)


def opencode_list_sessions(project_path: str | None, limit: int | None = None) -> list[dict]:
    con = opencode_connect()
    if con is None:
        return []
    cwd = os.path.abspath(project_path or os.getcwd())
    q = "SELECT id, title, time_updated FROM session WHERE directory = ? ORDER BY time_updated DESC"
    if limit:
        q += f" LIMIT {int(limit)}"
    with con:
        rows = con.execute(q, (cwd,)).fetchall()
    return [{"id": r[0], "title": r[1], "updated": _iso(r[2], ms=True), "updated_epoch": (r[2] or 0) / 1000} for r in rows]


def opencode_resolve_id(project_path: str | None, session_ref: str) -> str | None:
    con = opencode_connect()
    if con is None:
        return None
    cwd = os.path.abspath(project_path or os.getcwd())
    if session_ref in (None, "latest"):
        row = con.execute(
            "SELECT id FROM session WHERE directory = ? ORDER BY time_updated DESC LIMIT 1", (cwd,)
        ).fetchone()
    else:
        row = con.execute(
            "SELECT id FROM session WHERE directory = ? AND id LIKE ? ORDER BY time_updated DESC LIMIT 1",
            (cwd, session_ref + "%"),
        ).fetchone()
    return row[0] if row else None


def opencode_session_messages(session_id: str) -> list[dict]:
    con = opencode_connect()
    if con is None:
        return []
    messages = []
    for mid, data in con.execute(
        "SELECT id, data FROM message WHERE session_id = ? ORDER BY time_created ASC", (session_id,)
    ).fetchall():
        try:
            info = json.loads(data)
        except json.JSONDecodeError:
            info = {}
        parts = []
        for (pdata,) in con.execute(
            "SELECT data FROM part WHERE message_id = ? ORDER BY time_created ASC", (mid,)
        ).fetchall():
            try:
                parts.append(json.loads(pdata))
            except json.JSONDecodeError:
                continue
        messages.append({"info": info, "parts": parts})
    return messages


def opencode_known_projects() -> list[str]:
    con = opencode_connect()
    if con is None:
        return []
    rows = con.execute("SELECT DISTINCT directory FROM session").fetchall()
    return [r[0] for r in rows if r[0]]


def opencode_render_part(part: dict, full: bool, show_thinking: bool) -> str | None:
    ptype = part.get("type")
    if ptype == "text":
        return _trim(part.get("text", ""), full)
    if ptype == "reasoning":
        if not show_thinking:
            return None
        return "[thinking] " + _trim(part.get("text", ""), full)
    if ptype == "tool":
        state = part.get("state", {}) or {}
        inp = _trim(json.dumps(state.get("input", {}), ensure_ascii=False), full)
        out = state.get("output", state.get("status", ""))
        return f"[tool] {part.get('tool')}({inp}) => {_trim(str(out), full)}"
    if ptype in ("step-start", "step-finish"):
        return None
    return f"[{ptype}]"


def _opencode_message_text(message: dict) -> str:
    return " ".join(
        p.get("text", "") for p in message.get("parts", []) if isinstance(p, dict) and p.get("type") in ("text", "reasoning")
    )


def opencode_list(args) -> int:
    if opencode_connect() is None:
        print(f"opencode db not found: {opencode_db_path()}", file=sys.stderr)
        return 1
    sessions = opencode_list_sessions(args.project, args.limit)
    if not sessions:
        print(f"no opencode sessions for: {os.path.abspath(args.project or os.getcwd())}", file=sys.stderr)
        return 1
    for s in sessions:
        print(f"{s['id']}\t{s['updated']}\t{s['title']}")
    return 0


def opencode_show(args) -> int:
    session_id = opencode_resolve_id(args.project, args.session)
    if not session_id:
        print(f"session not found: {args.session!r} in {os.path.abspath(args.project or os.getcwd())}", file=sys.stderr)
        return 1
    for message in opencode_session_messages(session_id):
        info = message["info"]
        role = info.get("role")
        if role not in ("user", "assistant"):
            continue
        if args.role != "all" and role != args.role:
            continue
        ts = (info.get("time") or {}).get("created")
        print(f"--- {role} {_iso(ts, ms=True)} ---")
        for part in message["parts"]:
            rendered = opencode_render_part(part, args.full, args.thinking)
            if rendered is not None:
                print(rendered)
    return 0


def opencode_search(args) -> int:
    project_paths = opencode_known_projects() if args.all_projects else [os.path.abspath(args.project or os.getcwd())]
    hits = 0
    for project_path in project_paths:
        for s in opencode_list_sessions(project_path):
            for message in opencode_session_messages(s["id"]):
                if message["info"].get("role") not in ("user", "assistant"):
                    continue
                text = _opencode_message_text(message)
                if args.keyword.lower() in text.lower():
                    ts = (message["info"].get("time") or {}).get("created")
                    print(f"{s['id']}\t{_iso(ts, ms=True)}\t{' '.join(text.split())[:200]}")
                    hits += 1
    if hits == 0:
        print("no matches", file=sys.stderr)
        return 1
    return 0


# ---------------------------------------------------------------------------
# agy (Antigravity CLI): ~/.gemini/antigravity-cli
# conversation index: conversation_summaries.db (sqlite, always current)
# when present, else the cache/conversation_metadata.json snapshot (can lag
# a just-created session).
# Per-conversation content is a sqlite db of protobuf-serialized blobs with
# no available schema, so content here is recovered by pulling printable
# UTF-8 runs out of the raw bytes: readable, but lossy and noisy — labelled
# as such in the output.
# ---------------------------------------------------------------------------

AGY_BASE = os.path.expanduser(os.path.join("~", ".gemini", "antigravity-cli"))
_CONTROL_RE = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f�]+")
_UUID_RE = re.compile(r"^[0-9a-fA-F-]{16,}$")


def _agy_metadata() -> dict:
    db_path = os.path.join(AGY_BASE, "conversation_summaries.db")
    if os.path.isfile(db_path):
        try:
            con = _sqlite_ro_connect(db_path)
            with con:
                rows = con.execute(
                    "SELECT conversation_id, title, preview, last_modified_time, workspace_uris FROM conversation_summaries"
                ).fetchall()
            out = {}
            for cid, title, preview, last_mod, uris_json in rows:
                try:
                    uris = json.loads(uris_json) if uris_json else []
                except json.JSONDecodeError:
                    uris = []
                out[cid] = {
                    "summary": {
                        "ID": cid,
                        "Title": title,
                        "Preview": preview,
                        "UpdatedAt": str(last_mod),
                        "WorkspaceURIs": uris,
                    }
                }
            if out:
                return out
        except sqlite3.Error:
            pass

    path = os.path.join(AGY_BASE, "cache", "conversation_metadata.json")
    if not os.path.isfile(path):
        return {}
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f).get("conversations", {})
    except (json.JSONDecodeError, OSError):
        return {}


def _agy_workspace_match(uris, project_path: str | None) -> bool:
    target = os.path.abspath(project_path or os.getcwd())
    for uri in uris or []:
        p = uri[len("file://") :] if uri.startswith("file://") else uri
        if os.path.abspath(p) == target:
            return True
    return False


def agy_list_sessions(project_path: str | None, limit: int | None = None) -> list[dict]:
    out = []
    for cid, entry in _agy_metadata().items():
        summary = entry.get("summary", {})
        if not _agy_workspace_match(summary.get("WorkspaceURIs"), project_path):
            continue
        updated = summary.get("UpdatedAt", "")
        out.append(
            {
                "id": cid,
                "title": summary.get("Title") or summary.get("Preview") or "(untitled)",
                "updated": updated,
                "updated_epoch": _epoch_from_iso_utc(updated),
            }
        )
    out.sort(key=lambda s: s["updated_epoch"], reverse=True)
    return out[:limit] if limit else out


def agy_resolve_id(project_path: str | None, session_ref: str) -> str | None:
    sessions = agy_list_sessions(project_path)
    if not sessions:
        return None
    if session_ref in (None, "latest"):
        return sessions[0]["id"]
    for s in sessions:
        if s["id"] == session_ref or s["id"].startswith(session_ref):
            return s["id"]
    return None


def _extract_readable(blob: bytes, min_len: int = 6) -> list[str]:
    text = blob.decode("utf-8", errors="replace")
    out = []
    for chunk in _CONTROL_RE.split(text):
        chunk = chunk.strip()
        if len(chunk) >= min_len and any(ch.isalnum() for ch in chunk) and not _UUID_RE.match(chunk):
            out.append(chunk)
    return out


def agy_turn_lines(session_id: str, full: bool) -> list[tuple[int, list[str]]]:
    """Fallback only: protobuf steps, heuristically de-noised. See agy_turns."""
    db_path = os.path.join(AGY_BASE, "conversations", f"{session_id}.db")
    if not os.path.isfile(db_path):
        return []
    con = _sqlite_ro_connect(db_path)
    rows = con.execute("SELECT idx, step_payload FROM steps ORDER BY idx ASC").fetchall()
    out = []
    for idx, payload in rows:
        if not payload:
            continue
        fragments = _extract_readable(payload)
        if not fragments:
            continue
        text = " / ".join(fragments)
        out.append((idx, [_trim(text, full)]))
    return out


# ~/.gemini/antigravity-cli/brain/<id>/.system_generated/logs/transcript.jsonl
# is a clean per-step JSON log — verified present for every conversation
# found on this machine, so it is the primary content source; the protobuf
# extraction above is kept only as a fallback for a conversation that lacks
# this file.
_AGY_ROLE_BY_SOURCE = {"USER_EXPLICIT": "user", "MODEL": "assistant", "SYSTEM": "system"}
_AGY_STEP_KEYS = {"step_index", "source", "type", "status", "created_at", "content", "thinking", "tool_calls"}


def _agy_transcript_path(session_id: str) -> str:
    return os.path.join(AGY_BASE, "brain", session_id, ".system_generated", "logs", "transcript.jsonl")


def _agy_render_step(entry: dict, full: bool, show_thinking: bool) -> str | None:
    pieces = []
    if show_thinking and entry.get("thinking"):
        pieces.append("[thinking] " + _trim(entry["thinking"], full))
    content = entry.get("content")
    if isinstance(content, str) and content:
        pieces.append(_trim(content, full))
    for tc in entry.get("tool_calls") or []:
        if isinstance(tc, dict):
            pieces.append(f"[tool_call] {tc.get('name')}({_trim(json.dumps(tc.get('args', {}), ensure_ascii=False), full)})")
    if not pieces:
        extra = {k: v for k, v in entry.items() if k not in _AGY_STEP_KEYS}
        if extra:
            pieces.append(f"[{entry.get('type')}] {_trim(json.dumps(extra, ensure_ascii=False), full)}")
    return "\n".join(pieces) if pieces else None


def agy_turns(session_id: str, full: bool, show_thinking: bool) -> list[dict]:
    path = _agy_transcript_path(session_id)
    if not os.path.isfile(path):
        return []
    turns = []
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            text = _agy_render_step(entry, full, show_thinking)
            if text is None:
                continue
            turns.append({"role": _AGY_ROLE_BY_SOURCE.get(entry.get("source"), "note"), "ts": entry.get("created_at", ""), "text": text})
    return turns


def agy_list(args) -> int:
    sessions = agy_list_sessions(args.project, args.limit)
    if not sessions:
        print(f"no agy conversations for: {os.path.abspath(args.project or os.getcwd())}", file=sys.stderr)
        return 1
    for s in sessions:
        print(f"{s['id']}\t{s['updated']}\t{s['title']}")
    return 0


def agy_show(args) -> int:
    session_id = agy_resolve_id(args.project, args.session)
    if not session_id:
        print(f"session not found: {args.session!r} in {os.path.abspath(args.project or os.getcwd())}", file=sys.stderr)
        return 1
    turns = agy_turns(session_id, args.full, args.thinking)
    if turns:
        for turn in turns:
            if args.role != "all" and turn["role"] != args.role:
                continue
            print(f"--- {turn['role']} {turn['ts']} ---")
            print(turn["text"])
        return 0
    print("# no transcript.jsonl for this session; falling back to protobuf heuristic extraction (lossy)", file=sys.stderr)
    lines = agy_turn_lines(session_id, args.full)
    if not lines:
        print("no turn content found for this session", file=sys.stderr)
        return 1
    for idx, ls in lines:
        print(f"--- step {idx} ---")
        for line in ls:
            print(line)
    return 0


def agy_known_projects() -> list[str]:
    dirs = set()
    for entry in _agy_metadata().values():
        for uri in entry.get("summary", {}).get("WorkspaceURIs", []) or []:
            p = uri[len("file://") :] if uri.startswith("file://") else uri
            if os.path.isdir(p):
                dirs.add(os.path.abspath(p))
    return sorted(dirs)


def agy_search(args) -> int:
    project_paths = agy_known_projects() if args.all_projects else [os.path.abspath(args.project or os.getcwd())]
    hits = 0
    for project_path in project_paths:
        for s in agy_list_sessions(project_path):
            turns = agy_turns(s["id"], full=True, show_thinking=False)
            if turns:
                for turn in turns:
                    if args.keyword.lower() in turn["text"].lower():
                        print(f"{s['id']}\t{turn['ts']}\t{' '.join(turn['text'].split())[:200]}")
                        hits += 1
                continue
            for idx, lines in agy_turn_lines(s["id"], full=True):
                text = " ".join(lines)
                if args.keyword.lower() in text.lower():
                    print(f"{s['id']}\tstep {idx}\t{text[:200]}")
                    hits += 1
    if hits == 0:
        print("no matches", file=sys.stderr)
        return 1
    return 0


# ---------------------------------------------------------------------------
# kiro (Kiro CLI): ~/.kiro/sessions/cli/<uuid>.json (+ matching .jsonl).
# The .json sidecar carries "cwd" directly — that's the project filter.
# The .jsonl sidecar is a clean per-turn log, one JSON object per line:
#   {"kind": "Prompt", "data": {"content": [{"kind": "text", "data": "..."}], "meta": {"timestamp": ...}}}
#   {"kind": "AssistantMessage", "data": {"content": [{"kind": "thinking"|"text", ...}], "meta": {...}}}
# Verified against a real populated session. (A separate, empty-in-practice
# ~/.kiro/sessions/<hash>/sess_*/session.json mechanism exists too, but
# every example found on this machine had zero turns, so it's not used
# here.)
# ---------------------------------------------------------------------------


def _kiro_cli_sessions() -> list[tuple[str, dict]]:
    pattern = os.path.expanduser(os.path.join("~", ".kiro", "sessions", "cli", "*.json"))
    out = []
    for path in glob.glob(pattern):
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
        except (json.JSONDecodeError, OSError):
            continue
        session_id = os.path.splitext(os.path.basename(path))[0]
        out.append((session_id, data))
    return out


def _kiro_jsonl_path(session_id: str) -> str:
    return os.path.expanduser(os.path.join("~", ".kiro", "sessions", "cli", f"{session_id}.jsonl"))


def _kiro_first_prompt_text(session_id: str, max_len: int = 80) -> str:
    path = _kiro_jsonl_path(session_id)
    if not os.path.isfile(path):
        return "(no messages)"
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for line in f:
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            if entry.get("kind") != "Prompt":
                continue
            text = _kiro_content_text(entry.get("data", {}).get("content"), full=True, show_thinking=False)
            return " ".join(text.split())[:max_len]
    return "(no messages)"


def kiro_list_sessions(project_path: str | None, limit: int | None = None) -> list[dict]:
    target = os.path.abspath(project_path or os.getcwd())
    out = []
    for session_id, data in _kiro_cli_sessions():
        if os.path.abspath(data.get("cwd", "")) != target:
            continue
        updated = data.get("updated_at") or data.get("created_at") or ""
        out.append(
            {
                "id": session_id,
                "title": data.get("title") or _kiro_first_prompt_text(session_id),
                "updated": updated,
                "updated_epoch": _epoch_from_iso_utc(updated),
            }
        )
    out.sort(key=lambda s: s["updated_epoch"], reverse=True)
    return out[:limit] if limit else out


def kiro_resolve_id(project_path: str | None, session_ref: str) -> str | None:
    sessions = kiro_list_sessions(project_path)
    if not sessions:
        return None
    if session_ref in (None, "latest"):
        return sessions[0]["id"]
    for s in sessions:
        if s["id"] == session_ref or s["id"].startswith(session_ref):
            return s["id"]
    return None


def _kiro_content_text(content, full: bool, show_thinking: bool) -> str:
    if isinstance(content, str):
        return _trim(content, full)
    if not isinstance(content, list):
        return ""
    pieces = []
    for item in content:
        if not isinstance(item, dict):
            continue
        kind = item.get("kind")
        if kind == "text":
            data = item.get("data")
            pieces.append(_trim(data if isinstance(data, str) else json.dumps(data, ensure_ascii=False), full))
        elif kind == "thinking":
            if not show_thinking:
                continue
            text = (item.get("data") or {}).get("text") or "[redacted]"
            pieces.append("[thinking] " + _trim(text, full))
        else:
            pieces.append(f"[{kind}] " + _trim(json.dumps(item.get("data"), ensure_ascii=False), full))
    return "\n".join(p for p in pieces if p)


def kiro_turns(session_id: str, full: bool, show_thinking: bool) -> list[dict]:
    path = _kiro_jsonl_path(session_id)
    if not os.path.isfile(path):
        return []
    role_by_kind = {"Prompt": "user", "AssistantMessage": "assistant"}
    turns = []
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            role = role_by_kind.get(entry.get("kind"))
            if role is None:
                continue
            data = entry.get("data", {})
            text = _kiro_content_text(data.get("content"), full, show_thinking)
            ts = (data.get("meta") or {}).get("timestamp")
            turns.append({"role": role, "ts": _iso(ts, ms=False), "text": text})
    return turns


def kiro_list(args) -> int:
    sessions = kiro_list_sessions(args.project, args.limit)
    if not sessions:
        print(f"no kiro-cli sessions for: {os.path.abspath(args.project or os.getcwd())}", file=sys.stderr)
        return 1
    for s in sessions:
        print(f"{s['id']}\t{s['updated']}\t{s['title']}")
    return 0


def kiro_show(args) -> int:
    session_id = kiro_resolve_id(args.project, args.session)
    if not session_id:
        print(f"session not found: {args.session!r} in {os.path.abspath(args.project or os.getcwd())}", file=sys.stderr)
        return 1
    turns = kiro_turns(session_id, args.full, args.thinking)
    if not turns:
        print("no turn content found for this session", file=sys.stderr)
        return 1
    for turn in turns:
        if args.role != "all" and turn["role"] != args.role:
            continue
        print(f"--- {turn['role']} {turn['ts']} ---")
        print(turn["text"])
    return 0


def kiro_known_projects() -> list[str]:
    dirs = set()
    for _, data in _kiro_cli_sessions():
        cwd = data.get("cwd")
        if cwd and os.path.isdir(cwd):
            dirs.add(os.path.abspath(cwd))
    return sorted(dirs)


def kiro_search(args) -> int:
    project_paths = kiro_known_projects() if args.all_projects else [os.path.abspath(args.project or os.getcwd())]
    hits = 0
    for project_path in project_paths:
        for s in kiro_list_sessions(project_path):
            for turn in kiro_turns(s["id"], full=True, show_thinking=False):
                if args.keyword.lower() in turn["text"].lower():
                    print(f"{s['id']}\t{turn['ts']}\t{' '.join(turn['text'].split())[:200]}")
                    hits += 1
    if hits == 0:
        print("no matches", file=sys.stderr)
        return 1
    return 0


# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------

SOURCES = ["claude", "opencode", "agy", "kiro"]
LIST_SESSIONS = {
    "claude": claude_list_sessions,
    "opencode": opencode_list_sessions,
    "agy": agy_list_sessions,
    "kiro": kiro_list_sessions,
}
DISPATCH = {
    "list": {"claude": claude_list, "opencode": opencode_list, "agy": agy_list, "kiro": kiro_list},
    "show": {"claude": claude_show, "opencode": opencode_show, "agy": agy_show, "kiro": kiro_show},
    "search": {"claude": claude_search, "opencode": opencode_search, "agy": agy_search, "kiro": kiro_search},
}


def cmd_timeline(args) -> int:
    sources = SOURCES if args.source == "all" else [args.source]
    entries = []
    for src in sources:
        for s in LIST_SESSIONS[src](args.project):
            entries.append((s.get("updated_epoch") or 0, src, s["title"]))
    entries.sort(key=lambda e: e[0])
    if args.limit:
        entries = entries[-args.limit :]
    if not entries:
        print(f"no sessions found for: {os.path.abspath(args.project or os.getcwd())}", file=sys.stderr)
        return 1
    for epoch, src, title in entries:
        ts = datetime.datetime.fromtimestamp(epoch).isoformat(timespec="seconds") if epoch else "?"
        print(f"{ts}\t{src}\t{title}")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_list = sub.add_parser("list", help="list sessions for a project")
    p_list.add_argument("--project", default=None, help="project path (default: cwd)")
    p_list.add_argument("--source", choices=SOURCES, default="claude")
    p_list.add_argument("--limit", type=int, default=20)

    p_show = sub.add_parser("show", help="show one session's turns")
    p_show.add_argument("session", nargs="?", default="latest", help="session id, id prefix, or 'latest'")
    p_show.add_argument("--project", default=None)
    p_show.add_argument("--source", choices=SOURCES, default="claude")
    p_show.add_argument("--role", choices=["all", "user", "assistant"], default="all")
    p_show.add_argument("--full", action="store_true", help="do not truncate content")
    p_show.add_argument("--thinking", action="store_true", help="include thinking/reasoning blocks")

    p_search = sub.add_parser("search", help="search sessions for a keyword")
    p_search.add_argument("keyword")
    p_search.add_argument("--project", default=None)
    p_search.add_argument("--source", choices=SOURCES, default="claude")
    p_search.add_argument("--all-projects", action="store_true")

    p_timeline = sub.add_parser("timeline", help="merge every source's sessions into one chronological, titles-only timeline")
    p_timeline.add_argument("--project", default=None)
    p_timeline.add_argument("--source", choices=[*SOURCES, "all"], default="all")
    p_timeline.add_argument("--limit", type=int, default=None, help="keep only the N most recent entries")

    args = parser.parse_args()
    if args.cmd == "timeline":
        sys.exit(cmd_timeline(args))
    sys.exit(DISPATCH[args.cmd][args.source](args))


if __name__ == "__main__":
    main()
