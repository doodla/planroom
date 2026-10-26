# `@planroom/ui`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** design tokens (TS and CSS variables, light and dark), view-model rows, hotkey assignment, truncation, the braille raster, the machine stepper.
- **May depend on:** `@planroom/contract`.
- **Must not:** React, the DOM, Node APIs.
- **How to test it:** vitest; `machine_step_parity` against Rust golden outputs.
