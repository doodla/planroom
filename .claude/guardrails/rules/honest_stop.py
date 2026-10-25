"""Don't end a turn while work is owed and unrecorded.

While a build is in progress (feature_list.json exists), a stop is blocked if
there are uncommitted changes, unpushed commits, or failing tier A entries with
no '## Blocked' section in progress.md saying why. It blocks at most once per
stop (stop_hook_active), so it can never loop.
"""
from honesty import features, git, root

EVENTS = {"Stop"}


def check(hook, opts):
    if hook.get("stop_hook_active"):
        return []
    base = root(hook)
    fl = base / opts.get("feature_list", "feature_list.json")
    if not fl.exists():
        return []
    problems = []
    if any(l.strip() for l in git(base, "status", "--porcelain").splitlines()):
        problems.append("there are uncommitted changes: commit them (with progress.md updated).")
    ahead = git(base, "rev-list", "--count", "@{u}..HEAD").strip()
    if ahead and ahead != "0":
        problems.append(f"{ahead} commit(s) aren't pushed.")
    try:
        failing = [i for i, f in features(fl.read_text()).items() if f.get("tier") == "A" and f.get("passes") is not True]
    except ValueError:
        failing = ["(feature_list.json doesn't parse)"]
    if failing and not blocked_section(base / opts.get("progress", "progress.md")):
        problems.append(f"{len(failing)} tier A entr{'y' if len(failing) == 1 else 'ies'} still fail "
                        f"({', '.join(failing[:5])}), and progress.md has no '## Blocked' section saying what "
                        "only the human can unblock. Keep working, or record the blocker.")
    return problems


def blocked_section(path):
    try:
        lines = path.read_text().splitlines()
    except OSError:
        return False
    start = next((i for i, l in enumerate(lines) if l.strip().lower() == "## blocked"), None)
    if start is None:
        return False
    body = []
    for l in lines[start + 1:]:
        if l.startswith("## "):
            break
        body.append(l)
    return any(l.strip() and l.strip().lower() not in ("none", "- none", "nothing") for l in body)
