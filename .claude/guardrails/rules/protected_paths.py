"""Human-owned files stay human-owned, and frozen files stay frozen.

- `locked` globs (guardrails, hook settings, KICKOFF.md) can't be edited or
  written by an agent at all. Propose a change in progress.md instead.
- `write_once` globs (examples/TRUTH.json) can be created, but never changed
  once they exist. Changing a frozen truth goes through a decision record and a
  person, who runs that session with GUARDRAILS_OFF=protected_paths,commit_gate.

Shell writes are caught where they're visible (redirects, sed -i, tee, rm, mv,
cp, git checkout/restore). A script that writes files itself isn't visible
here; commit_gate catches it on the staged diff.
"""
import os
import re
import shlex

from honesty import matches, rel, root, strip_heredocs

EVENTS = {"PreToolUse"}
TOOLS = {"Edit", "Write", "MultiEdit", "NotebookEdit", "Bash"}

WRITERS = {"rm", "mv", "tee", "truncate", "chmod", "chown", "unlink", "shred"}
COPIERS = {"cp", "ln", "install", "rsync"}  # only the last argument is written
INPLACE = re.compile(r"^(?:sed|perl|gsed)$")
GIT_WRITERS = {"checkout", "restore", "rm", "mv", "apply", "am", "stash"}
REDIRECT = re.compile(r"(?<![<0-9&])(?:\d?>{1,2}|&>)\s*([^\s;&|]+)")


def check(hook, opts):
    base = root(hook)
    locked, once = opts.get("locked", []), opts.get("write_once", [])
    tool, ti = hook.get("tool_name"), hook.get("tool_input") or {}
    if tool == "Bash":
        targets = shell_targets(ti.get("command", ""), hook.get("cwd") or str(base))
    else:
        targets = [ti.get("file_path") or ti.get("notebook_path") or ""]
    problems = []
    for t in targets:
        r = rel(t if t.startswith("/") else f"{base}/{t}", base)
        if not r:
            continue
        if matches(r, locked):
            problems.append(f"{r} is human-owned. Don't change it; write the change you want under "
                            "'## Proposals for the human' in progress.md and carry on.")
        elif matches(r, once) and (base / r).exists():
            problems.append(f"{r} is frozen. Write a decision record with the evidence, get an independent "
                            "reviewer's approval, and list it under '## Proposals for the human' in progress.md; "
                            "a person applies it. Meanwhile treat the affected scenario as blocked, not passed.")
    return sorted(set(problems))


def shell_targets(command, cwd):
    """Absolute paths a shell command visibly writes to, following `cd` between segments."""
    targets = []
    for segment in re.split(r"&&|\|\||;|\||\n", strip_heredocs(command)):
        try:
            words = shlex.split(segment)
        except ValueError:
            words = segment.split()
        if not words:
            continue
        cmd, args = words[0].rsplit("/", 1)[-1], words[1:]
        if cmd == "cd" and args:
            cwd = os.path.normpath(os.path.join(cwd, os.path.expanduser(args[0])))
            continue
        found = [t.strip("'\"") for t in REDIRECT.findall(segment)]
        paths = [a for a in args if not a.startswith("-") and not REDIRECT.fullmatch(a)]
        if cmd in COPIERS:
            found += paths[-1:]
        elif cmd == "dd":
            found += [a[3:] for a in args if a.startswith("of=")]
        elif cmd in WRITERS or (INPLACE.match(cmd) and any(a.startswith("-i") for a in args)) \
                or (cmd == "git" and args and args[0] in GIT_WRITERS):
            found += paths
        targets += [os.path.normpath(os.path.join(cwd, os.path.expanduser(t))) for t in found]
    return targets
