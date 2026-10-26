# `@planroom/e2e`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** Playwright specs and the browser shims.
- **May depend on:** `@planroom/contract`, `@playwright/test`.
- **Must not:** product code (it drives the built canvas).
- **How to test it:** it is the test.

## Layout

- `playwright.config.ts`: one worker, results under `target/e2e/`.
- `src/`: the specs (`*.spec.ts`, run by `cargo xtask e2e`, which builds the canvas first), their fixtures, and `server.ts`, a stand-in for `planroom serve` that serves `target/canvas/` and answers from `tests/golden/cli/` with the server's auth rules. `cargo xtask e2e --screens` also writes `docs/screenshots/`. `engine-watch.ts` is vitest's global setup: it runs the built `target/debug/planroom watch --json` once on the fixture and provides its output to `packages/mod/src/watch-engine.test.ts`, which has no Node APIs to run it itself.
