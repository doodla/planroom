# Planroom

## Subagents: model and effort

Always set `model` and `effort` on Agent calls and workflow `agent()` calls (a hook enforces it; forks are exempt).

- Default to your own model. Go down only for mechanical work where mistakes are obvious; go up only when mistakes are costly and subtle.
- Scale effort to the judgment needed.
- On a bad result, retry one level up, not the same.
- Use aliases (`haiku`, `sonnet`, `opus`). A fitting `.claude/agents/` type can stand in for both.

## Subagents: checking the work

- Treat subagent output as a claim. Verify what you act on or report (rerun the test, read the cited lines).
- For anything costly to get wrong, use an independent checker: give it the work, not your conclusion. Checkers are never weaker than what they check.
- As a subagent, separate what you verified from what you assumed, and cite the evidence (command output, `file:line`).
