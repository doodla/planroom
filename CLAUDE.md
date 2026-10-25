# Planroom

## Subagents: model and effort

Always set `model` and `effort` on Agent calls and workflow `agent()` calls (a hook enforces it; forks are exempt).

- Default to your own model. Go down only for mechanical work where mistakes are obvious; go up only when mistakes are costly and subtle.
- Scale effort to the judgment needed.
- Checkers are never weaker than what they check.
- On a bad result, retry one level up, not the same.
- Use aliases (`haiku`, `sonnet`, `opus`). A fitting `.claude/agents/` type can stand in for both.
