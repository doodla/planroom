"""After a compaction or resume, put the contract back in front of the agent.

Only active while a build is in progress (feature_list.json exists), so
ordinary sessions in this repo aren't affected. The card is short on purpose:
it points at the full text instead of repeating it.
"""
import json
from pathlib import Path

from honesty import features, git, root

EVENTS = {"SessionStart"}
SOURCES = {"compact", "resume", "startup"}


def check(hook, opts):
    if hook.get("source") not in SOURCES:
        return []
    base = root(hook)
    fl = next(iter(sorted(base.glob(opts.get("feature_list", "feature_list.json")))), None)
    if not fl:
        return []
    lines = [f"Context restored after '{hook.get('source')}'. This is the contract you are working under."]
    lines += [f"- {c}" for c in opts.get("card", [])]
    lines += feature_summary(fl)
    lines += progress_now(base / opts.get("progress", "progress.md"))
    branch = git(base, "branch", "--show-current").strip()
    dirty = len([l for l in git(base, "status", "--porcelain").splitlines() if l.strip()])
    ahead = git(base, "rev-list", "--count", "@{u}..HEAD").strip() or "?"
    lines.append(f"Git: branch {branch or '?'}, {dirty} uncommitted path(s), {ahead} unpushed commit(s).")
    log = git(base, "log", "--oneline", "-3").strip()
    if log:
        lines.append("Last commits:\n" + log)
    return ["\n".join(lines)]


def feature_summary(path):
    try:
        items = features(path.read_text())
    except (OSError, json.JSONDecodeError):
        return [f"{path.name} doesn't parse; fix it first."]
    out = []
    for tier in ("A", "B", "C"):
        fs = [f for f in items.values() if f.get("tier") == tier]
        if fs:
            done = sum(1 for f in fs if f.get("passes") is True)
            out.append(f"Tier {tier}: {done}/{len(fs)} passing.")
    failing = [fid for fid, f in items.items() if f.get("tier") == "A" and f.get("passes") is not True]
    if failing:
        out.append("Failing tier A: " + ", ".join(failing[:15]) + (" …" if len(failing) > 15 else ""))
    return out


def progress_now(path, limit=40):
    """The '## Now' section of progress.md, or its head if there's none."""
    try:
        text = path.read_text()
    except OSError:
        return [f"No {path.name} yet. Create it, with a '## Now' section, before doing anything else."]
    lines = text.splitlines()
    start = next((i for i, l in enumerate(lines) if l.strip().lower() == "## now"), None)
    if start is None:
        body = lines[:limit]
    else:
        end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith("## ")), len(lines))
        body = lines[start:end][:limit]
    return [f"From {path.name}:", *body]
