# `@planroom/canvas`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the canvas app, both transports, views, the ELK worker, motion.
- **May depend on:** `@planroom/contract`, `@planroom/ui`, React, `@xyflow/react`, `elkjs`, `zustand`, `motion`, `radix-ui`.
- **Must not:** `@planroom/mod`; engine internals; fetching any host but its own origin.
- **How to test it:** vitest, Playwright e2e.
