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
| `protected_paths` | Agents can't edit `locked` files (these guardrails, `.claude/settings.json`, `KICKOFF.md`), and can't change `write_once` files (`examples/TRUTH.json`) once they exist. Shell writes are caught where they're visible. |
| `test_integrity` | No new skip/only/todo markers in test files; dropped assertions warn. `feature_list.json` entries can't be removed or have their tier lowered, and a tier A/B entry can't flip to passing unless its `evidence` paths exist. |
| `release_actions` | No publishing, release edits, tag pushes or deletes, force pushes, repo-setting changes or `--no-verify`. Only real invocations count, not text in an echo or heredoc. |
| `commit_gate` | At `git commit`, re-runs the protected-path, test and feature-list checks on the actual diff, catching writes that bypassed the edit tools. |
| `honest_stop` | While a build is running (`feature_list.json` exists): a stop is blocked if work is uncommitted or unpushed, or if tier A entries fail with no `## Blocked` section in `progress.md`. At most once per stop. |
| `context_restore` | While a build is running: after startup, resume or compaction, injects the contract card from `config.toml`, tier status, failing tier A entries, the `## Now` section of `progress.md`, and git state. |

### What these hooks are, and aren't

They're tripwires for drift and honest mistakes in a long unattended run, not a sandbox. They see tool calls and shell commands, not what a script does once it runs, so a determined agent could write a file from Python or call git from a subprocess. `commit_gate` narrows that gap by checking the real diff at commit time. Treat any attempt to get around a guardrail as a bug in the agent's behaviour, and look at `logs/decisions.jsonl` when reviewing a run.

### Human-owned files

These guardrails, `.claude/settings.json` and `KICKOFF.md` are locked against agents, so a long autonomous run can't loosen its own rules. To change them, a person starts a session with `GUARDRAILS_OFF=protected_paths,commit_gate` (locally, in the shell that launches `claude`; in a cloud environment, as an environment variable), makes the change, and starts a normal session again.

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

`runner.render` implements output for PreToolUse (deny or context), SessionStart (context only) and Stop (block with a reason). A rule can mark a problem with `honesty.soft()` so it only warns, even in enforce mode.
