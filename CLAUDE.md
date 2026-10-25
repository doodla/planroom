# Planroom

## Subagents: model and effort

Every subagent you spawn (Agent tool calls and `agent()` in workflow scripts) must set `model` and `effort` explicitly. A guardrail hook blocks spawns that don't. This file is the explicit instruction to set them. Forks inherit both, so they're exempt.

Choose them for the step in front of you, not by habit:

- **Start from your own model.** Step down a tier only when the work is mechanical, fully specified, and a mistake would be cheap to spot. Step up only when a wrong answer would be costly and hard to catch.
- **Effort follows how much judgment the step needs:** low for lookups and mechanical changes, higher for design, debugging and verification.
- **A checker is never weaker than what it checks.** Verifiers, reviewers and judges get at least the model and effort of the work they review.
- **Escalate, don't repeat.** If a subagent's output is wrong or thin, retry one tier or effort level up, not the same settings again.
- **Use model aliases** (`haiku`, `sonnet`, `opus`), not versioned IDs.
- **Prefer a named agent type** from `.claude/agents/` when one fits. Its definition already sets model and effort.
