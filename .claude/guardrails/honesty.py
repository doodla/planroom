"""Shared checks for the honesty rules: protected paths, test markers, feature_list.json.

The rules call these on proposed file contents (edit time) and on the staged
diff (commit time), so a write that bypasses the edit tools is still caught.
"""
import fnmatch
import json
import os
import re
import subprocess
from pathlib import Path

TIER_RANK = {"A": 0, "B": 1, "C": 2}


class Soft(str):
    """A problem that only warns, even from a rule in enforce mode."""


def soft(message):
    return Soft(message)

# Markers that switch a test off or narrow a run to a few tests.
SKIP_MARKERS = re.compile(
    r"\b(?:it|test|describe|context|suite)\.(?:skip|only|todo|fixme)\b"
    r"|\b(?:xit|xdescribe|xtest|fit|fdescribe)\s*\("
    r"|@pytest\.mark\.(?:skip|skipif|xfail)\b"
    r"|@unittest\.(?:skip|skipIf|skipUnless|expectedFailure)\b"
    r"|\bt\.Skip(?:Now|f)?\s*\("
    r"|#\[ignore\]"
)
ASSERTIONS = re.compile(r"\bexpect\s*\(|\bassert\w*\b|\bt\.(?:Error|Fatal)f?\s*\(")
TEST_GLOBS = ("*.test.*", "*.spec.*", "test_*.py", "*_test.py", "*_test.go", "tests/*", "*/tests/*",
              "e2e/*", "*/e2e/*", "__tests__/*", "*/__tests__/*")


def root(hook):
    return Path(os.environ.get("CLAUDE_PROJECT_DIR") or hook.get("cwd") or ".").resolve()


def rel(path, base):
    p = Path(path)
    if not p.is_absolute():
        p = base / p
    try:
        return p.resolve().relative_to(base).as_posix()
    except ValueError:
        return None  # outside the project


def matches(relpath, globs):
    return any(fnmatch.fnmatch(relpath, g) for g in globs)


def is_test_file(relpath):
    return matches(relpath, TEST_GLOBS) and not relpath.endswith((".md", ".json", ".snap"))


def count(pattern, text):
    return len(pattern.findall(text or ""))


def test_problems(relpath, old, new):
    """Problems with changing a test file from `old` to `new` text: (denials, warnings)."""
    if not is_test_file(relpath):
        return [], []
    denials, warnings = [], []
    added = count(SKIP_MARKERS, new) - count(SKIP_MARKERS, old)
    if added > 0:
        denials.append(f"{relpath}: adds {added} skip/only/todo marker(s). Fix the code or the test, "
                       "never switch a test off. If a scenario is infeasible, follow the substitution rule in KICKOFF.md.")
    dropped = count(ASSERTIONS, old) - count(ASSERTIONS, new)
    if dropped > 0:
        warnings.append(f"{relpath}: removes {dropped} assertion(s). That's fine for a real refactor; "
                        "it's not fine for getting green. Say which in the commit message.")
    return denials, warnings


def features(text):
    """Parse feature_list.json text into {id: entry}. Accepts a list or {"features": [...]}."""
    if not text or not text.strip():
        return {}
    data = json.loads(text)
    items = data.get("features", []) if isinstance(data, dict) else data
    return {str(f.get("id")): f for f in items if isinstance(f, dict) and f.get("id") is not None}


def evidence_paths(entry):
    ev = entry.get("evidence") or []
    return [ev] if isinstance(ev, str) else [e for e in ev if isinstance(e, str)]


def feature_problems(old_text, new_text, base, exists=None):
    """Denials for a feature_list.json change. `exists(path)` says whether evidence is present."""
    exists = exists or (lambda p: (base / p).exists())
    try:
        old, new = features(old_text), features(new_text)
    except (json.JSONDecodeError, AttributeError) as e:
        return [f"feature_list.json doesn't parse: {e}"]
    problems = []
    for fid, before in old.items():
        after = new.get(fid)
        if after is None:
            problems.append(f"feature {fid} was removed. Entries are never deleted; record a gap or an approved substitution instead.")
            continue
        b, a = TIER_RANK.get(str(before.get("tier")), 9), TIER_RANK.get(str(after.get("tier")), 9)
        if a > b:
            problems.append(f"feature {fid} tier lowered from {before.get('tier')} to {after.get('tier')}. "
                            "Tiers only change through an approved decision record, applied by a human.")
    for fid, after in new.items():
        before = old.get(fid, {})
        if after.get("passes") is True and before.get("passes") is not True and after.get("tier") in ("A", "B"):
            ev = evidence_paths(after)
            missing = [p for p in ev if not exists(p)]
            if not ev:
                problems.append(f"feature {fid} marked passing with no `evidence`. List the test file and any recorded output.")
            elif missing:
                problems.append(f"feature {fid} marked passing, but its evidence doesn't exist: {', '.join(missing)}.")
    return problems


HEREDOC = re.compile(r"<<-?\s*(['\"]?)(\w+)\1[^\n]*\n.*?\n\s*\2\s*(?=\n|$)", re.S)


def strip_heredocs(command):
    """Drop heredoc bodies, so text fed to cat or python isn't read as commands."""
    return HEREDOC.sub("", command)


def git(base, *args):
    out = subprocess.run(["git", "-C", str(base), *args], capture_output=True, text=True, timeout=5)
    return out.stdout if out.returncode == 0 else ""
