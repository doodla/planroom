# Guardrails

Claude Code hooks that keep agents on track. Built for unattended auto-mode runs.

Every hook goes through `runner.py`. It runs the enabled rules from `config.toml`
and can only **deny**, **warn** (add context for the agent), or stay **silent**.
It never returns `allow` (that could skip auto mode's own permission checks) or
`ask` (nobody is there to answer).

## Rules

| Rule | Does |
|---|---|
| `subagent_model_effort` | Agent/Task calls and workflow `agent(...)` calls must set `model` and `effort`, either inline or through an agent definition in `.claude/agents/`. Forks are exempt. |
| `context_restore` | After startup, resume or compaction, replays `.claude/carry.md`: the notes the agent chose to keep across compactions, capped at 4,000 characters. |

## Operating

- **Modes** (`config.toml`): `enforce` denies, `warn` tells the agent but lets the call through, `off` skips the rule. Start new rules in `warn`.
- **Rule crashes**: with `on_error = "closed"`, a crash is reported at the rule's mode: an `enforce` rule denies the call, a `warn` rule only warns, so an unproven rule can't block everything. With `"open"`, the crash is only logged. If the runner itself crashes or times out, `onFailure: "block"` in `settings.json` blocks the call.
- **Escape hatch**: `GUARDRAILS_OFF=rule_a,rule_b` disables rules for a session.
- **Decision log**: every warn and deny is appended to `logs/decisions.jsonl` (gitignored), or to `$GUARDRAILS_LOG` if set.

## Adding a rule

1. Add `rules/<name>.py` with `EVENTS`, optional `TOOLS`, and `check(hook, opts) -> list[str]` (an empty list means fine).
2. Add `[rules.<name>]` to `config.toml`, starting with `mode = "warn"`.
3. Make sure `.claude/settings.json` routes that event and those tools to the runner. A test checks the PreToolUse matcher.
4. Add tests and run `python3 .claude/guardrails/tests/test_guardrails.py`.

`runner.render` implements output for PreToolUse (deny or context) and SessionStart (context only). Add the output shape for other events (such as Stop) when the first rule for them arrives.
