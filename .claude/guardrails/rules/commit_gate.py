"""Re-check the honesty rules on what a commit actually contains.

Edit-time rules can't see files written by scripts. At `git commit`, this
looks at the real diff (staged, plus tracked changes for `-a`/`--all`) and
denies the commit if it touches locked or frozen files, adds skip markers to
tests, or makes a dishonest feature_list.json change.
"""
import re
import shlex
from pathlib import Path

from honesty import feature_problems, git, matches, root, test_problems

EVENTS = {"PreToolUse"}
TOOLS = {"Bash"}

# git, any global options (-C dir, -c k=v, --git-dir=...), then commit.
COMMIT = re.compile(r"(?:^|&&|;|\|\||\n|\$\(|`)\s*(?:\S+=\S+\s+)*(?:\S*/)?git\s+"
                    r"(?:(?:-[Cc]\s+\S+|--[\w-]+(?:=\S+)?)\s+)*commit\b([^;&|\n]*)")


def check(hook, opts):
    m = COMMIT.search((hook.get("tool_input") or {}).get("command", ""))
    if not m:
        return []
    base = root(hook)
    try:
        flags = shlex.split(m.group(1))
    except ValueError:
        flags = m.group(1).split()
    all_tracked = any(f in ("-a", "--all") or (f.startswith("-") and not f.startswith("--") and "a" in f[1:])
                      for f in flags if f != "-m")
    changed = set(git(base, "diff", "--cached", "--name-only").split())
    if all_tracked:
        changed |= set(git(base, "diff", "--name-only").split())
    problems = []
    for r in sorted(changed):
        if matches(r, opts.get("locked", [])):
            problems.append(f"{r} is human-owned and can't be committed by an agent.")
        elif matches(r, opts.get("write_once", [])) and git(base, "ls-tree", "HEAD", r).strip():
            problems.append(f"{r} is frozen; a change to it goes through a decision record and a person.")
        old = git(base, "show", f"HEAD:{r}")
        new = new_text(base, r, all_tracked)
        if Path(r).name == "feature_list.json":
            problems += feature_problems(old, new, base)
        else:
            problems += test_problems(r, old, new)[0]
    return problems


def new_text(base, r, all_tracked):
    if all_tracked:
        p = base / r
        return p.read_text() if p.exists() else ""
    return git(base, "show", f":{r}")
