"""Tests and the feature list only change in honest directions.

- Test files: adding skip/only/todo markers is denied; dropping assertions warns.
- feature_list.json: entries can't be removed, tiers can't be lowered, and a
  tier A/B entry can't flip to passing unless its `evidence` paths exist.
"""
from pathlib import Path

from honesty import feature_problems, rel, root, soft, test_problems

EVENTS = {"PreToolUse"}
TOOLS = {"Edit", "Write", "MultiEdit"}


def check(hook, opts):
    base = root(hook)
    ti = hook.get("tool_input") or {}
    path = ti.get("file_path") or ""
    r = rel(path, base)
    if not r:
        return []
    p = base / r
    old = p.read_text() if p.exists() else ""
    new = proposed(hook.get("tool_name"), ti, old)
    if new is None:
        return []
    if Path(r).name == "feature_list.json":
        return feature_problems(old, new, base)
    denials, warnings = test_problems(r, old, new)
    return denials + [soft(w) for w in warnings]


def proposed(tool, ti, old):
    """The file's text after this edit, or None if it can't be worked out."""
    if tool == "Write":
        return ti.get("content", "")
    edits = ti.get("edits") if tool == "MultiEdit" else [ti]
    text = old
    for e in edits or []:
        o, n = e.get("old_string", ""), e.get("new_string", "")
        if o and o not in text:
            return None  # the edit will fail on its own
        text = text.replace(o, n) if e.get("replace_all") else text.replace(o, n, 1)
    return text
