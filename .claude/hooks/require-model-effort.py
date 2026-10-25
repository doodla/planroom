#!/usr/bin/env python3
"""PreToolUse hook: every subagent spawn must pin an explicit model and effort.

- Agent/Task tool calls need `model` and `effort` in the tool input.
  Forks (subagent_type "fork") are exempt: they always inherit the parent's
  model and effort, so the fields would be ignored.
- Workflow scripts need `model:` and `effort:` in the options object of every
  agent(...) call. Options passed via a variable can't be checked, so they
  must be written inline.

Exit 2 blocks the call and sends stderr back to Claude so it can retry.
"""
import json
import re
import sys

AGENT_CALL = re.compile(r"(?<![\w$.])agent\s*\(")


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


def check_workflow(tool_input):
    script = tool_input.get("script")
    path = tool_input.get("scriptPath")
    if path:
        try:
            with open(path) as f:
                script = f.read()
        except OSError:
            return []  # let the Workflow tool report the bad path itself
    if not script:
        return []  # named/saved workflow: nothing to inspect here

    code = mask(script)
    problems = []
    for m in AGENT_CALL.finditer(code):
        call = call_text(code, m.end() - 1)
        missing = [k for k in ("model", "effort") if not re.search(rf"\b{k}\s*:", call)]
        if missing:
            line = script.count("\n", 0, m.start()) + 1
            snippet = " ".join(script[m.start() : m.end() - 1 + len(call)].split())[:80]
            problems.append(f"  line {line}: missing {', '.join(missing)} in {snippet}")
    return problems


def main():
    data = json.load(sys.stdin)
    tool = data.get("tool_name")
    tool_input = data.get("tool_input") or {}

    if tool in ("Agent", "Task"):
        if tool_input.get("subagent_type") == "fork":
            return 0
        missing = [k for k in ("model", "effort") if not tool_input.get(k)]
        if missing:
            print(
                f"Blocked: {tool} calls must set {' and '.join(missing)} explicitly. "
                "Pick a model (e.g. sonnet, opus, haiku) and an effort "
                "(low|medium|high|xhigh|max) that fit this task, then retry.",
                file=sys.stderr,
            )
            return 2

    elif tool == "Workflow":
        problems = check_workflow(tool_input)
        if problems:
            print(
                "Blocked: every agent(...) call in a workflow script must pass inline "
                "`model:` and `effort:` options (e.g. {model: 'sonnet', effort: 'low'}).\n"
                + "\n".join(problems),
                file=sys.stderr,
            )
            return 2

    return 0


if __name__ == "__main__":
    sys.exit(main())
