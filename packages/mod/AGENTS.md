# `@planroom/mod`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the mod: wiring, pane, band, `/room`, tools, guards, broker, agents.
- **May depend on:** `@planroom/contract`, `@planroom/ui`, the `claude-code` mod types.
- **Must not:** `@planroom/canvas`; the DOM or Node APIs; `import()`.
- **How to test it:** vitest (pure logic); `claude plugin test` on all four surfaces with golden CLI outputs.
