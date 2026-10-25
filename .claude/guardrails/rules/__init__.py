"""Guardrail rules.

Each module defines:
  EVENTS: set of hook event names it applies to
  TOOLS:  set of tool names it applies to (omit for all tools)
  check(hook: dict, opts: dict) -> list[str]  problems found; empty means fine
"""
