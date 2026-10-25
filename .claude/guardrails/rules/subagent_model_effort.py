"""Every subagent spawn must pin a model and an effort, explicitly or via its agent definition.

- Agent/Task calls: `model` and `effort` in the tool input, unless the
  subagent_type's definition sets them. Forks are exempt: they always inherit
  the parent's model and effort.
- Workflow scripts: each agent(...) call needs inline `model:` and `effort:`
  options, unless its inline `agentType:` names a definition that sets them.
  Options passed via a variable can't be checked, so they must be inline.
  Saved workflows run by `name` aren't checked (no script in the call).
"""
import os
import re

from agent_defs import pinned

EVENTS = {"PreToolUse"}
TOOLS = {"Agent", "Task", "Workflow"}

FIELDS = ("model", "effort")
AGENT_CALL = re.compile(r"(?<![\w$.])agent\s*\(")
AGENT_TYPE = re.compile(r"""(["']?)\bagentType\1\s*:\s*(["'`])([^"'`]+)\2""")
HINT = "Pick a model (e.g. sonnet, opus, haiku) and an effort (low|medium|high|xhigh|max) that fit the task."


def check(hook, opts):
    tool = hook.get("tool_name")
    tool_input = hook.get("tool_input") or {}
    if tool in ("Agent", "Task"):
        return check_agent(tool, tool_input, hook)
    if tool == "Workflow":
        return check_workflow(tool_input, hook)
    return []


def check_agent(tool, tool_input, hook):
    agent_type = tool_input.get("subagent_type")
    if agent_type == "fork":
        return []
    covered = pinned(agent_type, hook)
    missing = [k for k in FIELDS if not tool_input.get(k) and k not in covered]
    if not missing:
        return []
    return [f"{tool} call must set {' and '.join(missing)}. {HINT}"]


def check_workflow(tool_input, hook):
    script = tool_input.get("script")
    path = tool_input.get("scriptPath")
    if path:
        if not os.path.isabs(path) and hook.get("cwd"):
            path = os.path.join(hook["cwd"], path)
        try:
            with open(path) as f:
                script = f.read()
        except OSError:
            return []  # the Workflow tool reports a bad path itself
    if not script:
        return []

    code = mask(script)
    calls = []  # (where "agent" starts, where its "(" is, end of the call)
    for m in AGENT_CALL.finditer(code):
        start = m.end() - 1
        calls.append((m.start(), start, start + len(call_text(code, start))))

    problems = []
    pins = {}
    for name_at, start, end in calls:
        # Blank out nested agent(...) calls so their options can't satisfy this one.
        chars = list(code[start:end])
        for inner_at, _, inner_end in calls:
            if start < inner_at and inner_end <= end:
                for j in range(inner_at - start, inner_end - start):
                    if chars[j] != "\n":
                        chars[j] = " "
        call = "".join(chars)
        original = script[start:end]
        agent_type = inline_agent_type(original, call)
        if agent_type and agent_type not in pins:
            pins[agent_type] = pinned(agent_type, hook)
        covered = pins.get(agent_type, set())
        missing = [k for k in FIELDS if k not in covered and not re.search(rf"\b{k}\s*:", call)]
        if missing:
            line = script.count("\n", 0, name_at) + 1
            snippet = " ".join(script[name_at:end].split())[:80]
            problems.append(f"line {line}: missing {', '.join(missing)} in {snippet}")
    if not problems:
        return []
    return [
        "Every agent(...) call in a workflow script must pass inline `model:` and `effort:` "
        "options (e.g. {model: 'sonnet', effort: 'low'}).",
        *problems,
    ]


def inline_agent_type(original, masked):
    """Return the agentType option written as code in this call, ignoring prompt text."""
    for m in AGENT_TYPE.finditer(original):
        if not masked[m.start()].isspace():  # key is code, not text inside a string
            return m.group(3)
    return None


def mask(src):
    """Blank out comments and string/template text, keeping length and newlines.

    Code inside template ${...} is kept. This stops parens or `model:` inside
    prompts, and agent(...) inside comments, from confusing the checks.
    """
    out = list(src)
    stack = []  # '"', "'", '`', or '{' for code inside ${...}

    def blank(a, b):
        for j in range(a, min(b, len(src))):
            if out[j] != "\n":
                out[j] = " "

    i = 0
    while i < len(src):
        c = src[i]
        ctx = stack[-1] if stack else None
        if ctx in ('"', "'", "`"):
            if c == "\\":
                blank(i, i + 2)
                i += 2
                continue
            if c == ctx:
                stack.pop()
            elif ctx == "`" and src.startswith("${", i):
                stack.append("{")
                i += 2
                continue
            else:
                blank(i, i + 1)
        elif src.startswith("//", i):
            nl = src.find("\n", i)
            nl = len(src) if nl == -1 else nl
            blank(i, nl)
            i = nl
            continue
        elif src.startswith("/*", i):
            e = src.find("*/", i + 2)
            e = len(src) if e == -1 else e + 2
            blank(i, e)
            i = e
            continue
        elif c in ('"', "'", "`"):
            stack.append(c)
        elif c == "{" and ctx == "{":
            stack.append("{")
        elif c == "}" and ctx == "{":
            stack.pop()
        i += 1
    return "".join(out)


def call_text(code, start):
    """Return masked code from the opening paren at `start` to its match."""
    depth = 0
    for i in range(start, len(code)):
        if code[i] == "(":
            depth += 1
        elif code[i] == ")":
            depth -= 1
            if depth == 0:
                return code[start : i + 1]
    return code[start:]
