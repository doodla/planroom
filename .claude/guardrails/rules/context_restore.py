"""After a compaction or resume, replay the notes the agent chose to keep.

The agent owns the carry file (.claude/carry.md by default) and writes whatever
it needs to survive a compaction. This rule only reads it back. It's capped in
size, so it stays notes rather than a second transcript.
"""
import os
from pathlib import Path

EVENTS = {"SessionStart"}
SOURCES = {"compact", "resume", "startup"}


def check(hook, opts):
    if hook.get("source") not in SOURCES:
        return []
    base = Path(os.environ.get("CLAUDE_PROJECT_DIR") or hook.get("cwd") or ".")
    path = opts.get("carry", ".claude/carry.md")
    limit = opts.get("carry_limit", 4000)
    try:
        text = (base / path).read_text().strip()
    except OSError:
        return []
    if not text:
        return []
    note = ""
    if len(text) > limit:
        note = f"\n[{path} is {len(text) - limit} characters over its {limit}-character limit and was cut here. Trim it.]"
        text = text[:limit]
    return [f"Notes you chose to carry across compactions ({path}). Trust them over your memory, "
            f"and keep them current and short:\n{text}{note}"]
