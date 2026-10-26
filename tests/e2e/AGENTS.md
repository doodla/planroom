# `@planroom/e2e`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** Playwright specs and the browser shims.
- **May depend on:** `@planroom/contract`, `@playwright/test`.
- **Must not:** product code (it drives the built canvas).
- **How to test it:** it is the test.

## Layout

- `playwright.config.ts`: one worker, results under `target/e2e/`.
- `src/`: the specs (`*.spec.ts`, run by `cargo xtask e2e`, which builds the canvas, the engine with the canvas embedded and the fixture first) and their fixtures. `engine.ts` copies the built fixture, scans the copy and starts the real `planroom serve` on it, signing in with the login page's code and then with codes registered through `serve.json`'s control token; `surface.spec.ts` and `quality.spec.ts` run against it. `standin.ts` is a stand-in for `planroom serve` with the real auth rules, answering from the engine's goldens in `tests/golden/cli/` with an overlay laid over them, for `standin.spec.ts` alone: the states the engine can't produce yet. `cargo xtask e2e --screens` also writes `docs/screenshots/`.
- `overlays/`: test-only overlays on the goldens, one per milestone that makes their state real (`m1-…`, `m2-…`), each naming its golden, its milestone and why. A spec moves to the real engine, and its overlay goes, once the engine produces the state. Never hand-edit `tests/golden/` instead.
