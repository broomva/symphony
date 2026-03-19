#!/usr/bin/env python3
"""
conversation-history.py — Bridge .entire/ conversation logs to Obsidian knowledge graph.

Parses Claude Code session logs and transcript files to generate
Obsidian-compatible markdown documents per conversation session,
with wikilinks, frontmatter, and a Map of Content (MOC) index.

Usage:
    python3 scripts/conversation-history.py [--output docs/conversations] [--limit N]
"""

import json
import re
import sys
import argparse
from collections import defaultdict
from datetime import datetime
from pathlib import Path

# ── Paths ──────────────────────────────────────────────────────────────────────
REPO_ROOT = Path(__file__).resolve().parent.parent
ENTIRE_LOG = REPO_ROOT / ".entire" / "logs" / "entire.log"
DEFAULT_OUTPUT = REPO_ROOT / "docs" / "conversations"


def _resolve_transcripts_dir() -> Path:
    """Derive the Claude Code transcripts directory from the repo path.

    Claude Code stores transcripts at:
      ~/.claude/projects/{repo_path_with_slashes_as_dashes}/{session}.jsonl

    The project key is the absolute repo path with '/' replaced by '-'.
    Since the path starts with '/', the first char becomes '-' automatically.
    This works for any user/machine.
    """
    project_key = str(REPO_ROOT).replace("/", "-")
    return Path.home() / ".claude" / "projects" / project_key


TRANSCRIPTS_DIR = _resolve_transcripts_dir()


# ── Transcript Cache ──────────────────────────────────────────────────────────
_transcript_cache: dict[str, dict] = {}


def _get_transcript(sid: str) -> dict:
    """Get parsed transcript, using cache if available."""
    if sid in _transcript_cache:
        return _transcript_cache[sid]
    transcript_path = TRANSCRIPTS_DIR / f"{sid}.jsonl"
    transcript = parse_transcript(transcript_path)
    _transcript_cache[sid] = transcript
    return transcript


def _synthetic_meta(session_id: str, transcript: dict) -> dict:
    """Create synthetic session metadata from JSONL transcript when .entire/ is unavailable."""
    user_msgs = [e for e in transcript.get("timeline", []) if e.get("kind") == "user"]
    turns = [{"start": e.get("timestamp", "")} for e in user_msgs]

    return {
        "starts": [transcript.get("first_timestamp", "")],
        "ends": [transcript.get("last_timestamp", "")],
        "turns": turns,
        "subagents": [],
        "checkpoints": [],
        "commits": [],
        "attributions": [],
        "phases": [],
        "first_seen": transcript.get("first_timestamp"),
        "last_seen": transcript.get("last_timestamp"),
        "branch": transcript.get("git_branch"),
        "total_messages": transcript.get("total_messages", 0),
        "_source": "jsonl",
    }


# ── Event Log Parser ───────────────────────────────────────────────────────────
def parse_event_log(log_path: Path) -> dict:
    """Parse .entire/logs/entire.log into per-session metadata."""
    sessions = defaultdict(lambda: {
        "starts": [],
        "ends": [],
        "turns": [],
        "subagents": [],
        "checkpoints": [],
        "commits": [],
        "attributions": [],
        "phases": [],
        "first_seen": None,
        "last_seen": None,
        "branch": None,
    })

    with open(log_path, "r") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue

            sid = entry.get("session_id", "")
            if not sid:
                continue

            ts = entry.get("time", "")
            msg = entry.get("msg", "")
            event = entry.get("event", "")

            session = sessions[sid]

            # Track time bounds
            if ts:
                if session["first_seen"] is None or ts < session["first_seen"]:
                    session["first_seen"] = ts
                if session["last_seen"] is None or ts > session["last_seen"]:
                    session["last_seen"] = ts

            # Lifecycle events
            if event == "SessionStart":
                session["starts"].append(ts)
                ref = entry.get("session_ref", "")
                if ref:
                    session["transcript_ref"] = ref
            elif event == "SessionEnd":
                session["ends"].append(ts)
            elif event == "TurnStart":
                session["turns"].append({"start": ts})
            elif event == "TurnEnd":
                if session["turns"]:
                    session["turns"][-1]["end"] = ts

            # Subagents
            if event == "SubagentStart":
                session["subagents"].append({
                    "tool_use_id": entry.get("tool_use_id", ""),
                    "start": ts,
                    "type": entry.get("subagent_type", ""),
                })
            elif event == "SubagentEnd":
                agent_id = entry.get("agent_id", "")
                tool_id = entry.get("tool_use_id", "")
                for sa in reversed(session["subagents"]):
                    if sa.get("tool_use_id") == tool_id:
                        sa["end"] = ts
                        sa["agent_id"] = agent_id
                        break

            # Checkpoints
            if "checkpoint" in msg:
                session["checkpoints"].append({
                    "time": ts,
                    "type": entry.get("checkpoint_type", ""),
                    "modified": entry.get("modified_files", 0),
                    "new": entry.get("new_files", 0),
                    "deleted": entry.get("deleted_files", 0),
                    "shadow_branch": entry.get("shadow_branch", ""),
                    "subagent_type": entry.get("subagent_type", ""),
                })

            # Attribution
            if "attribution" in msg:
                session["attributions"].append({
                    "time": ts,
                    "agent_lines": entry.get("agent_lines", 0),
                    "human_added": entry.get("human_added", 0),
                    "human_modified": entry.get("human_modified", 0),
                    "files_touched": entry.get("files_touched", 0),
                    "agent_pct": entry.get("agent_percentage", 0),
                })

            # Phase transitions
            if "phase transition" in msg:
                session["phases"].append({
                    "time": ts,
                    "from": entry.get("from", ""),
                    "to": entry.get("to", ""),
                })

            # Commits
            if "commit" in msg.lower() and "prepare-commit-msg" in msg:
                session["commits"].append({
                    "time": ts,
                    "checkpoint_id": entry.get("checkpoint_id", ""),
                })

    return dict(sessions)


# Patterns that indicate a message is internal/system, not a real user prompt
_SYSTEM_MSG_PATTERNS = [
    "<task-notification>",
    "<task-id>",
    "<system-reminder>",
    "<local-command-caveat>",
    "[Request interrupted",
    "toolu_",           # Raw tool use IDs leaking into content
    "/private/tmp/claude",  # Internal file paths
]


# ── Transcript Parser ──────────────────────────────────────────────────────────
def parse_transcript(jsonl_path: Path, max_user_msgs: int = 50) -> dict:
    """Extract conversation content from a .jsonl transcript file.

    Builds a chronological timeline of events, grouped into 'turns':
    each turn starts with a user message and includes all assistant
    text blocks, tool calls, and tool results until the next user message.
    """
    result: dict = {
        "timeline": [],       # Chronological list of {type, ...} events
        "tools_used": set(),
        "files_touched": set(),
        "git_branch": None,
        "version": None,
        "total_messages": 0,
        "first_timestamp": None,
        "last_timestamp": None,
    }

    if not jsonl_path.exists():
        return result

    all_entries: list[dict] = []

    try:
        with open(jsonl_path, "r") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    entry = json.loads(line)
                except json.JSONDecodeError:
                    continue
                all_entries.append(entry)
    except Exception as e:
        result["parse_error"] = str(e)
        return result

    result["total_messages"] = len(all_entries)

    user_count = 0
    for entry in all_entries:
        ts = entry.get("timestamp", "")

        if ts:
            if result["first_timestamp"] is None or ts < result["first_timestamp"]:
                result["first_timestamp"] = ts
            if result["last_timestamp"] is None or ts > result["last_timestamp"]:
                result["last_timestamp"] = ts

        if entry.get("gitBranch"):
            result["git_branch"] = entry["gitBranch"]
        if entry.get("version"):
            result["version"] = entry["version"]

        entry_type = entry.get("type", "")

        # ── User messages (filter out system/tool-result noise) ──
        if entry_type == "user":
            # Skip tool result messages (internal tool→assistant feedback)
            if entry.get("toolUseResult") is not None:
                continue

            msg = entry.get("message", {})
            content = msg.get("content", "")
            text = _extract_text(content)

            if not text or _is_system_message(text):
                continue

            if user_count < max_user_msgs:
                result["timeline"].append({
                    "kind": "user",
                    "text": text[:2000],
                    "timestamp": ts,
                })
                user_count += 1

        # ── Assistant entries: extract ALL text blocks + tool calls ──
        elif entry_type == "assistant":
            msg = entry.get("message", {})
            content = msg.get("content", "")

            if isinstance(content, list):
                for block in content:
                    if not isinstance(block, dict):
                        continue
                    btype = block.get("type", "")

                    if btype == "text":
                        t = block.get("text", "").strip()
                        if t and len(t) > 10:
                            result["timeline"].append({
                                "kind": "assistant_text",
                                "text": t[:2000],
                                "timestamp": ts,
                            })

                    elif btype == "tool_use":
                        tool_name = block.get("name", "")
                        if tool_name:
                            result["tools_used"].add(tool_name)
                            tool_input = block.get("input", {})
                            desc = _summarize_tool_call(tool_name, tool_input)
                            details = _tool_detail_lines(tool_name, tool_input)
                            result["timeline"].append({
                                "kind": "tool_call",
                                "tool": tool_name,
                                "description": desc,
                                "details": details,
                                "timestamp": ts,
                            })

            elif isinstance(content, str) and content.strip() and len(content.strip()) > 10:
                result["timeline"].append({
                    "kind": "assistant_text",
                    "text": content.strip()[:2000],
                    "timestamp": ts,
                })

        # ── File operations from progress events ──
        elif entry_type == "progress":
            data = entry.get("data", {})
            if isinstance(data, dict):
                fp = data.get("file_path", "") or data.get("path", "")
                if fp:
                    fp = fp.replace(str(REPO_ROOT) + "/", "")
                    result["files_touched"].add(fp)

    # Convert sets to sorted lists
    result["tools_used"] = sorted(result["tools_used"])
    result["files_touched"] = sorted(list(result["files_touched"])[:50])

    return result


def _summarize_tool_call(tool_name: str, tool_input: dict) -> str:
    """Create a one-line summary of a tool call for the timeline."""
    rp = str(REPO_ROOT) + "/"
    if tool_name == "Bash":
        return tool_input.get("description", "") or tool_input.get("command", "")[:200]
    elif tool_name in ("Read", "Write", "Edit"):
        return tool_input.get("file_path", "").replace(rp, "")
    elif tool_name == "Grep":
        pattern = tool_input.get("pattern", "")
        path = tool_input.get("path", "").replace(rp, "")
        return f'"{pattern}" in {path}' if path else f'"{pattern}"'
    elif tool_name == "Glob":
        return tool_input.get("pattern", "")
    elif tool_name == "Agent":
        return tool_input.get("description", tool_input.get("prompt", "")[:120])
    elif tool_name in ("TaskCreate", "TaskUpdate"):
        return tool_input.get("description", tool_input.get("subject", ""))[:120]
    else:
        for key in ("description", "prompt", "command", "query", "pattern"):
            if key in tool_input:
                return str(tool_input[key])[:120]
        return ""


def _tool_detail_lines(tool_name: str, tool_input: dict) -> list[str]:
    """Return detail lines for a tool call to show inside a collapsible block."""
    rp = str(REPO_ROOT) + "/"
    lines: list[str] = []

    if tool_name == "Bash":
        cmd = tool_input.get("command", "")
        desc = tool_input.get("description", "")
        if desc:
            lines.append(f"*{desc}*")
        if cmd:
            cmd_preview = cmd[:500].replace("\n", " && ")
            lines.append(f"`{cmd_preview}`")

    elif tool_name == "Read":
        fp = tool_input.get("file_path", "").replace(rp, "")
        if fp:
            lines.append(f"`{fp}`")
        offset = tool_input.get("offset")
        limit = tool_input.get("limit")
        if offset or limit:
            parts = []
            if offset:
                parts.append(f"offset: {offset}")
            if limit:
                parts.append(f"limit: {limit}")
            lines.append(f"Range: {', '.join(parts)}")

    elif tool_name == "Write":
        fp = tool_input.get("file_path", "").replace(rp, "")
        content = tool_input.get("content", "")
        if fp:
            lines.append(f"`{fp}`")
        if content:
            preview = content[:500].replace("\n", " ").replace("`", "'")
            lines.append(f"Content: `{preview}{'...' if len(content) > 500 else ''}`")
            if len(content) > 500:
                lines.append(f"({len(content)} chars total)")

    elif tool_name == "Edit":
        fp = tool_input.get("file_path", "").replace(rp, "")
        old = tool_input.get("old_string", "")
        new = tool_input.get("new_string", "")
        if fp:
            lines.append(f"`{fp}`")
        if old:
            old_preview = old[:500].replace("\n", " ").replace("`", "'")
            lines.append(f"Old: `{old_preview}{'...' if len(old) > 500 else ''}`")
        if new:
            new_preview = new[:500].replace("\n", " ").replace("`", "'")
            lines.append(f"New: `{new_preview}{'...' if len(new) > 500 else ''}`")

    elif tool_name == "Grep":
        pattern = tool_input.get("pattern", "")
        path = tool_input.get("path", "").replace(rp, "")
        mode = tool_input.get("output_mode", "")
        if pattern:
            lines.append(f"Pattern: `{pattern}`")
        if path:
            lines.append(f"Path: `{path}`")
        if mode:
            lines.append(f"Mode: {mode}")

    elif tool_name == "Glob":
        pattern = tool_input.get("pattern", "")
        path = tool_input.get("path", "").replace(rp, "")
        if pattern:
            lines.append(f"Pattern: `{pattern}`")
        if path:
            lines.append(f"In: `{path}`")

    elif tool_name == "Agent":
        desc = tool_input.get("description", "")
        prompt = tool_input.get("prompt", "")
        sat = tool_input.get("subagent_type", "")
        bg = tool_input.get("run_in_background", False)
        if sat:
            lines.append(f"Type: **{sat}**")
        if desc:
            lines.append(f"Task: {desc}")
        if prompt:
            prompt_preview = prompt[:600].replace("\n", " ")
            lines.append(f"Prompt: {prompt_preview}")
        if bg:
            lines.append("*(background)*")

    elif tool_name == "TaskCreate":
        subj = tool_input.get("subject", "")
        desc = tool_input.get("description", "")
        if subj:
            lines.append(f"**{subj}**")
        if desc:
            desc_preview = desc[:500].replace("\n", " ")
            lines.append(desc_preview)

    elif tool_name == "TaskUpdate":
        tid = tool_input.get("taskId", "")
        status = tool_input.get("status", "")
        blocked = tool_input.get("addBlockedBy", [])
        if tid:
            lines.append(f"Task: #{tid}")
        if status:
            lines.append(f"Status: {status}")
        if blocked:
            lines.append(f"Blocked by: {blocked}")

    elif tool_name == "Skill":
        skill = tool_input.get("skill", "")
        args = tool_input.get("args", "")
        if skill:
            lines.append(f"Skill: `{skill}`")
        if args:
            lines.append(f"Args: {args[:200]}")

    else:
        # Generic: show all input fields
        for k, v in list(tool_input.items())[:5]:
            val = str(v)[:200]
            lines.append(f"{k}: {val}")

    return lines


def _is_system_message(text: str) -> bool:
    """Check if a message is internal/system noise, not a real user prompt."""
    for pattern in _SYSTEM_MSG_PATTERNS:
        if pattern in text:
            return True
    # Skip messages that are just very short acknowledgements from tool results
    if len(text) < 5:
        return True
    return False


def _extract_text(content) -> str:
    """Extract plain text from message content (string or content blocks)."""
    if isinstance(content, str):
        return content.strip()
    if isinstance(content, list):
        parts = []
        for block in content:
            if isinstance(block, dict) and block.get("type") == "text":
                parts.append(block.get("text", ""))
            elif isinstance(block, str):
                parts.append(block)
        return " ".join(parts).strip()
    return ""




# ── Helpers ────────────────────────────────────────────────────────────────────
def _ts_short(ts: str) -> str:
    """Extract HH:MM from an ISO timestamp."""
    if not ts:
        return ""
    try:
        dt = datetime.fromisoformat(ts.replace("Z", "+00:00"))
        return dt.strftime("%H:%M")
    except (ValueError, TypeError):
        return ""


# ── Callout-safe text ──────────────────────────────────────────────────────────
def _callout_safe(text: str) -> str:
    """Sanitize text for use inside Obsidian callout blocks (> prefixed).

    Issues addressed:
    - Markdown headers (## Foo) render as real headers, breaking the callout
    - HTML/XML tags (<task-notification>) render as raw markup
    - Blank lines inside callouts break the callout block
    """
    result_lines = []
    for line in text.split("\n"):
        # Strip XML/HTML tags (task-notification, system-reminder, etc.)
        line = re.sub(r'<[^>]+>', '', line)

        # Convert markdown headers to bold text (headers break callouts)
        line = re.sub(r'^(#{1,6})\s+(.+)$', r'**\2**', line)

        # Preserve blank lines inside callout with empty quote marker
        if not line.strip():
            result_lines.append("")
        else:
            result_lines.append(line)

    return "\n".join(result_lines)


# ── Markdown Generator ─────────────────────────────────────────────────────────
def generate_session_doc(session_id: str, meta: dict, transcript: dict) -> str:
    """Generate an Obsidian markdown document for a single session."""
    # Determine date from first_seen or transcript
    date_str = ""
    ts = meta.get("first_seen") or transcript.get("first_timestamp", "")
    if ts:
        try:
            dt = datetime.fromisoformat(ts.replace("Z", "+00:00"))
            date_str = dt.strftime("%Y-%m-%d")
        except (ValueError, TypeError):
            date_str = ts[:10] if len(ts) >= 10 else ""

    short_id = session_id[:8]
    branch = transcript.get("git_branch", "") or ""
    version = transcript.get("version", "") or ""
    turn_count = len(meta.get("turns", []))
    tools = transcript.get("tools_used", [])
    files = transcript.get("files_touched", [])
    timeline = transcript.get("timeline", [])

    # Find first user message in timeline for title
    first_user_text = ""
    for ev in timeline:
        if ev.get("kind") == "user":
            first_user_text = ev.get("text", "")
            break

    # Derive a title from first user message
    title = f"Session {short_id}"
    if first_user_text:
        first_msg = first_user_text[:80].replace("\n", " ").strip()
        first_msg = re.sub(r'[#\[\]|`]', '', first_msg).strip()
        if first_msg:
            title = first_msg[:60] + ("..." if len(first_msg) > 60 else "")

    # Compute duration
    duration = ""
    start_ts = meta.get("first_seen", "")
    end_ts = meta.get("last_seen", "")
    if start_ts and end_ts:
        try:
            s = datetime.fromisoformat(start_ts.replace("Z", "+00:00"))
            e = datetime.fromisoformat(end_ts.replace("Z", "+00:00"))
            delta = e - s
            mins = int(delta.total_seconds() / 60)
            if mins >= 60:
                duration = f"{mins // 60}h {mins % 60}m"
            else:
                duration = f"{mins}m"
        except (ValueError, TypeError):
            pass

    # Build tags
    tags = ["stimulus/conversations"]
    if branch:
        tag_branch = branch.replace("/", "-").replace("_", "-")
        tags.append(f"branch/{tag_branch}")

    # Subagent types used
    subagent_types = set()
    for sa in meta.get("subagents", []):
        if sa.get("type"):
            subagent_types.add(sa["type"])

    # Attribution summary
    total_agent_lines = sum(a.get("agent_lines", 0) for a in meta.get("attributions", []))
    total_human_modified = sum(a.get("human_modified", 0) for a in meta.get("attributions", []))
    total_files_touched = max((a.get("files_touched", 0) for a in meta.get("attributions", [])), default=0)

    lines = []

    # Frontmatter
    lines.append("---")
    lines.append(f"title: \"{title}\"")
    lines.append(f"description: Claude Code session {short_id} on {date_str}")
    lines.append("tags:")
    for tag in tags:
        lines.append(f"  - {tag}")
    lines.append("type: conversation")
    lines.append("status: active")
    lines.append(f"created: {date_str}")
    lines.append(f"updated: {date_str}")
    lines.append(f"session_id: {session_id}")
    if branch:
        lines.append(f"branch: {branch}")
    lines.append("related:")
    lines.append("  - \"[[Conversations]]\"")
    lines.append("  - \"[[CLAUDE]]\"")
    lines.append("---")
    lines.append("")

    # Header
    lines.append(f"# {title}")
    lines.append("")

    # Metadata table
    lines.append("| Field | Value |")
    lines.append("|-------|-------|")
    lines.append(f"| **Session** | `{session_id}` |")
    lines.append(f"| **Date** | {date_str} |")
    if duration:
        lines.append(f"| **Duration** | {duration} |")
    lines.append(f"| **Turns** | {turn_count} |")
    if branch:
        lines.append(f"| **Branch** | `{branch}` |")
    if version:
        lines.append(f"| **Claude Code** | v{version} |")
    lines.append(f"| **Messages** | {transcript.get('total_messages', 0)} |")
    if total_agent_lines or total_human_modified:
        lines.append(f"| **Agent lines** | {total_agent_lines} |")
        lines.append(f"| **Human modified** | {total_human_modified} |")
    lines.append("")

    # Tools used
    if tools:
        lines.append("## Tools Used")
        lines.append("")
        lines.append(", ".join(f"`{t}`" for t in tools))
        lines.append("")

    # Subagents
    if subagent_types:
        lines.append("## Subagents")
        lines.append("")
        for sat in sorted(subagent_types):
            lines.append(f"- {sat}")
        lines.append("")

    # Conversation thread — render full chronological timeline
    timeline = transcript.get("timeline", [])
    if timeline:
        lines.append("## Conversation Thread")
        lines.append("")

        # Group consecutive tool_call events to avoid clutter
        tool_batch: list[dict] = []

        def _flush_tools():
            """Render accumulated tool calls with nested collapsible details."""
            nonlocal tool_batch
            if not tool_batch:
                return
            lines.append("> [!example] Tool Calls")
            for tc in tool_batch:
                desc = tc.get("description", "")
                details = tc.get("details", [])
                tool_label = f"**{tc['tool']}**"
                if desc:
                    tool_label += f" — {desc}"

                if details:
                    # Nested collapsible callout per tool with details
                    lines.append(f">> [!note] {tool_label}")
                    for dl in details:
                        # Prefix every line for nested callout (multi-line values)
                        for sub_line in dl.split("\n"):
                            lines.append(f">> {sub_line}")
                else:
                    # Simple bullet if no details
                    lines.append(f"> - {tool_label}")
            lines.append("")
            tool_batch = []

        for event in timeline:
            kind = event.get("kind", "")

            if kind == "user":
                # Flush any pending tool calls before a new user message
                _flush_tools()

                ts_short = _ts_short(event.get("timestamp", ""))
                safe_text = _callout_safe(event["text"])
                header = f"**User** ({ts_short})" if ts_short else "**User**"
                lines.append(f"> [!quote] {header}")
                for lt in safe_text.split("\n"):
                    lines.append(f"> {lt}")
                lines.append("")

            elif kind == "assistant_text":
                # Flush tool calls before assistant text
                _flush_tools()

                safe_text = _callout_safe(event["text"])
                lines.append(f"> [!info] **Assistant**")
                for lt in safe_text.split("\n"):
                    lines.append(f"> {lt}")
                lines.append("")

            elif kind == "tool_call":
                tool_batch.append(event)

        # Flush any remaining tool calls
        _flush_tools()
        lines.append("")

    # Files touched
    if files:
        lines.append("## Files Touched")
        lines.append("")
        for fp in files[:30]:
            lines.append(f"- `{fp}`")
        if len(files) > 30:
            lines.append(f"- ... and {len(files) - 30} more")
        lines.append("")

    # Checkpoints
    if meta.get("commits"):
        lines.append("## Commits")
        lines.append("")
        for c in meta["commits"]:
            lines.append(f"- `{c.get('checkpoint_id', '')}` at {c.get('time', '')[:19]}")
        lines.append("")

    # Navigation
    lines.append("---")
    lines.append("")
    lines.append("*Part of [[Conversations]] | See [[CLAUDE]] for project invariants*")

    return "\n".join(lines)


def generate_moc(session_docs: list, output_dir: Path) -> str:
    """Generate the Conversations.md Map of Content."""
    lines = []
    lines.append("---")
    lines.append("title: Conversations")
    lines.append("description: Map of Content for Claude Code conversation history sessions")
    lines.append("tags:")
    lines.append("  - stimulus/conversations")
    lines.append("  - moc")
    lines.append("type: moc")
    lines.append("status: active")
    lines.append(f"created: {datetime.now().strftime('%Y-%m-%d')}")
    lines.append(f"updated: {datetime.now().strftime('%Y-%m-%d')}")
    lines.append("related:")
    lines.append("  - \"[[Documentation Hub]]\"")
    lines.append("  - \"[[CLAUDE]]\"")
    lines.append("  - \"[[AGENTS]]\"")
    lines.append("---")
    lines.append("")
    lines.append("# Conversations")
    lines.append("")
    lines.append("> [!info] Agent Session History")
    lines.append("> This directory contains Obsidian-compatible records of every Claude Code")
    lines.append("> conversation session in this project. Each document traces the prompts,")
    lines.append("> tool usage, files modified, and commits — linking agent work to the")
    lines.append("> knowledge graph.")
    lines.append("")
    lines.append(f"**Total sessions indexed**: {len(session_docs)}")
    lines.append("")

    # Group by date
    by_date = defaultdict(list)
    for doc in session_docs:
        by_date[doc["date"]].append(doc)

    for date in sorted(by_date.keys(), reverse=True):
        lines.append(f"## {date}")
        lines.append("")
        lines.append("| Session | Branch | Turns | Duration | Topic |")
        lines.append("|---------|--------|-------|----------|-------|")
        for doc in sorted(by_date[date], key=lambda d: d.get("time", "")):
            name = doc["filename"].replace(".md", "")
            branch = doc.get("branch", "—")
            turns = doc.get("turns", 0)
            duration = doc.get("duration", "—")
            topic = doc.get("title", "")[:50]
            lines.append(f"| [[{name}]] | `{branch}` | {turns} | {duration} | {topic} |")
        lines.append("")

    # Stats
    lines.append("## Statistics")
    lines.append("")
    total_turns = sum(d.get("turns", 0) for d in session_docs)
    branches = set(d.get("branch", "") for d in session_docs if d.get("branch"))
    lines.append(f"- **Total turns**: {total_turns}")
    lines.append(f"- **Branches worked on**: {len(branches)}")
    lines.append(f"- **Date range**: {min(d['date'] for d in session_docs) if session_docs else '—'} → {max(d['date'] for d in session_docs) if session_docs else '—'}")
    lines.append("")

    # Navigation
    lines.append("---")
    lines.append("")
    lines.append("*Part of [[Documentation Hub]] | Generated by `scripts/conversation-history.py`*")

    return "\n".join(lines)


# ── Main ───────────────────────────────────────────────────────────────────────
def main():
    parser = argparse.ArgumentParser(description="Bridge .entire/ logs to Obsidian knowledge graph")
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT), help="Output directory for conversation docs")
    parser.add_argument("--limit", type=int, default=0, help="Limit number of sessions to process (0 = all)")
    parser.add_argument("--force", action="store_true", help="Overwrite existing session docs")
    parser.add_argument("--dry-run", action="store_true", help="Print stats without writing files")
    args = parser.parse_args()

    output_dir = Path(args.output)

    # Source 1: Event log (optional — provides rich metadata)
    sessions = {}
    if ENTIRE_LOG.exists():
        print(f"📂 Parsing event log: {ENTIRE_LOG}")
        sessions = parse_event_log(ENTIRE_LOG)
        print(f"   Found {len(sessions)} sessions from .entire/ event log")
    else:
        print("⏭  .entire/ not configured — using JSONL-only mode")

    # Source 2: JSONL transcripts — discover sessions not in event log
    if TRANSCRIPTS_DIR.exists():
        jsonl_files = sorted(TRANSCRIPTS_DIR.glob("*.jsonl"))
        discovered = 0
        for jsonl_file in jsonl_files:
            sid = jsonl_file.stem
            if sid not in sessions:
                transcript = parse_transcript(jsonl_file)
                if transcript.get("total_messages", 0) >= 3:
                    sessions[sid] = _synthetic_meta(sid, transcript)
                    _transcript_cache[sid] = transcript
                    discovered += 1
        if discovered:
            print(f"   Discovered {discovered} sessions from JSONL transcripts")
    else:
        if not sessions:
            print(f"⏭  No transcripts at {TRANSCRIPTS_DIR} and no .entire/ logs — nothing to do.")
            sys.exit(0)

    print(f"   Total: {len(sessions)} sessions")

    # Filter to sessions with meaningful activity
    active_sessions = {}
    for sid, meta in sessions.items():
        turns = len(meta.get("turns", []))
        msgs = meta.get("total_messages", 0)
        if turns >= 1 or msgs >= 3:
            active_sessions[sid] = meta
    print(f"   {len(active_sessions)} active sessions")

    if args.limit:
        # Take the most recent N
        sorted_sids = sorted(
            active_sessions.keys(),
            key=lambda s: active_sessions[s].get("first_seen", ""),
            reverse=True,
        )[:args.limit]
        active_sessions = {s: active_sessions[s] for s in sorted_sids}
        print(f"   Limited to {len(active_sessions)} most recent")

    if args.dry_run:
        print("\n📊 Dry run — would generate:")
        for sid, meta in sorted(active_sessions.items(), key=lambda x: x[1].get("first_seen", "") or ""):
            turns = len(meta.get("turns", []))
            ts = (meta.get("first_seen") or "?")[:19]
            source = "jsonl" if meta.get("_source") == "jsonl" else "entire"
            print(f"   {sid[:8]} | {ts} | {turns} turns | {source}")
        print(f"\n   Total: {len(active_sessions)} session docs + 1 MOC")
        return

    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)

    session_docs_meta = []
    processed = 0

    for sid, meta in sorted(active_sessions.items(), key=lambda x: x[1].get("first_seen", "")):
        short_id = sid[:8]
        date_str = ""
        ts = meta.get("first_seen", "")
        if ts:
            try:
                dt = datetime.fromisoformat(ts.replace("Z", "+00:00"))
                date_str = dt.strftime("%Y-%m-%d")
            except (ValueError, TypeError):
                date_str = ts[:10] if len(ts) >= 10 else "unknown"

        filename = f"session-{date_str}-{short_id}.md"
        filepath = output_dir / filename

        # Skip existing unless --force
        if filepath.exists() and not args.force:
            # Still collect metadata for MOC
            session_docs_meta.append(_read_existing_meta(filepath, filename, sid, meta))
            continue

        # Parse transcript (use cache if available from discovery)
        transcript = _get_transcript(sid)

        # Generate doc
        doc_content = generate_session_doc(sid, meta, transcript)

        # Write
        filepath.write_text(doc_content, encoding="utf-8")
        processed += 1

        # Compute duration for MOC
        duration = ""
        start_ts = meta.get("first_seen", "") or transcript.get("first_timestamp", "")
        end_ts = meta.get("last_seen", "") or transcript.get("last_timestamp", "")
        if start_ts and end_ts:
            try:
                s = datetime.fromisoformat(start_ts.replace("Z", "+00:00"))
                e = datetime.fromisoformat(end_ts.replace("Z", "+00:00"))
                mins = int((e - s).total_seconds() / 60)
                duration = f"{mins // 60}h {mins % 60}m" if mins >= 60 else f"{mins}m"
            except (ValueError, TypeError):
                pass

        # Derive title from first user message in timeline
        title = f"Session {short_id}"
        for ev in transcript.get("timeline", []):
            if ev.get("kind") == "user":
                first = ev["text"][:60]
                first = re.sub(r'[#\[\]|`]', '', first).replace("\n", " ").strip()
                if first:
                    title = first
                break

        session_docs_meta.append({
            "filename": filename,
            "session_id": sid,
            "date": date_str,
            "time": ts or transcript.get("first_timestamp", ""),
            "branch": transcript.get("git_branch", "") or meta.get("branch", ""),
            "turns": len(meta.get("turns", [])),
            "duration": duration,
            "title": title,
        })

        sys.stdout.write(f"\r   Processed {processed} sessions...")
        sys.stdout.flush()

    print(f"\n   Wrote {processed} new session docs")

    # Generate MOC
    moc_content = generate_moc(session_docs_meta, output_dir)
    moc_path = output_dir / "Conversations.md"
    moc_path.write_text(moc_content, encoding="utf-8")
    print(f"   Wrote MOC: {moc_path.relative_to(REPO_ROOT)}")

    print(f"\n✅ Done. {len(session_docs_meta)} sessions indexed in {output_dir.relative_to(REPO_ROOT)}/")


def _read_existing_meta(filepath: Path, filename: str, sid: str, meta: dict) -> dict:
    """Read minimal metadata from an existing session doc for MOC generation."""
    content = filepath.read_text(encoding="utf-8")

    title = f"Session {sid[:8]}"
    branch = ""
    for line in content.split("\n"):
        if line.startswith("title:"):
            title = line.split(":", 1)[1].strip().strip('"')
        if line.startswith("branch:"):
            branch = line.split(":", 1)[1].strip()

    date_str = ""
    ts = meta.get("first_seen", "")
    if ts:
        try:
            dt = datetime.fromisoformat(ts.replace("Z", "+00:00"))
            date_str = dt.strftime("%Y-%m-%d")
        except (ValueError, TypeError):
            date_str = ts[:10] if len(ts) >= 10 else "unknown"

    duration = ""
    start_ts = meta.get("first_seen", "")
    end_ts = meta.get("last_seen", "")
    if start_ts and end_ts:
        try:
            s = datetime.fromisoformat(start_ts.replace("Z", "+00:00"))
            e = datetime.fromisoformat(end_ts.replace("Z", "+00:00"))
            mins = int((e - s).total_seconds() / 60)
            duration = f"{mins // 60}h {mins % 60}m" if mins >= 60 else f"{mins}m"
        except (ValueError, TypeError):
            pass

    return {
        "filename": filename,
        "session_id": sid,
        "date": date_str,
        "time": ts,
        "branch": branch,
        "turns": len(meta.get("turns", [])),
        "duration": duration,
        "title": title,
    }


if __name__ == "__main__":
    main()
