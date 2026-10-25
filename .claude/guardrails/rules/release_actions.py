"""Outward, hard-to-undo actions stay with the human.

Denies publishing packages, creating or editing GitHub releases, pushing or
deleting tags, force pushes, changing repo settings, and skipping git hooks.
Dry runs are fine. Only commands that actually run are checked: text inside an
echo, a heredoc or a quoted argument doesn't count.
"""
import re
import shlex

from honesty import strip_heredocs

EVENTS = {"PreToolUse"}
TOOLS = {"Bash"}

RULES = [
    (re.compile(r"\b(?:npm|pnpm|yarn|bun)\s+(?:\S+\s+)*publish\b(?!.*--dry-run)"), "publishing a package"),
    (re.compile(r"\bgh\s+release\s+(?:create|edit|delete|upload)\b"), "changing a GitHub release"),
    (re.compile(r"\bgh\s+repo\s+(?:edit|rename|delete|archive)\b"), "changing repo settings"),
    (re.compile(r"\bgh\s+api\b.*(?:-X|--method)\s*(?:PATCH|PUT|DELETE|POST)\b.*\brepos/[^/\s]+/[^/\s]+(?:/(?:releases|rulesets|branches/[^/\s]+/protection))?\s*(?:$|\s)"),
     "changing the repo through the API"),
    (re.compile(r"\bgit\s+push\b.*(?:--tags|--follow-tags|\brefs/tags/|--mirror|--delete|\s:\S)"), "pushing or deleting tags or refs"),
    (re.compile(r"\bgit\s+push\b.*\s(?:v\d[\w.\-]*)(?:\s|$)"), "pushing a version tag"),
    (re.compile(r"\bgit\s+push\b.*(?:\s-f\b|--force(?!-with-lease)\b|\s\+\S)"), "force-pushing"),
    (re.compile(r"\bgit\s+tag\s+(?:-d|--delete)\b"), "deleting a tag"),
    (re.compile(r"\bgit\b.*\s--no-verify\b|\bgit\s+commit\b.*\s-n\b"), "skipping git hooks"),
]


TOOLS_RUN = {"git", "gh", "npm", "pnpm", "yarn", "bun"}
ASSIGN = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")


def check(hook, opts):
    found = [what for seg in commands((hook.get("tool_input") or {}).get("command", ""))
             for rx, what in RULES if rx.search(seg)]
    if not found:
        return []
    return [f"{', '.join(sorted(set(found)))} is the human's call. Prepare it and list the exact command "
            "under '## Proposals for the human' in progress.md."]


def commands(command):
    """Each git/gh/package-manager invocation in the command, rebuilt from its words.

    Heredoc bodies are dropped first, so text fed to cat or python isn't mistaken
    for a command.
    """
    command = strip_heredocs(command)
    out = []
    for segment in re.split(r"&&|\|\||;|\||\n|\$\(|`", command):
        try:
            words = shlex.split(segment)
        except ValueError:
            words = segment.split()
        while words and (ASSIGN.match(words[0]) or words[0] in ("sudo", "env", "command", "exec")):
            words = words[1:]
        if words and words[0].rsplit("/", 1)[-1] in TOOLS_RUN:
            out.append(" ".join(words))
    return out

