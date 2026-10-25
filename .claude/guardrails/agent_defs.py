"""Look up subagent definitions (.claude/agents/*.md) and what their frontmatter pins."""
import os
import re
from pathlib import Path


def search_dirs(hook):
    project = os.environ.get("CLAUDE_PROJECT_DIR") or hook.get("cwd")
    dirs = []
    if project:
        dirs.append(Path(project) / ".claude" / "agents")
    dirs.append(Path.home() / ".claude" / "agents")
    return dirs


def frontmatter(text):
    m = re.match(r"---\s*\n(.*?)\n---", text, re.S)
    if not m:
        return {}
    fields = {}
    for line in m.group(1).splitlines():
        key, sep, value = line.partition(":")
        if sep:
            fields[key.strip()] = value.strip().strip("\"'")
    return fields


def pinned(agent_type, hook):
    """Return the subset of {"model", "effort"} the named agent's definition sets.

    Project definitions win over user ones, matching Claude Code's precedence.
    Built-in and plugin agent types aren't found here, so they pin nothing.
    """
    if not agent_type:
        return set()
    for d in search_dirs(hook):
        if not d.is_dir():
            continue
        for path in d.glob("*.md"):
            fields = frontmatter(path.read_text(errors="replace"))
            if fields.get("name", path.stem) == agent_type:
                return {k for k in ("model", "effort") if fields.get(k) and fields[k] != "inherit"}
    return set()
