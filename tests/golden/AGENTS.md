# `tests/golden`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the Rust tests' outputs that the mod tests and `tsc` read.
- **May depend on:** `cargo xtask gen --golden`.
- **Must not:** hand edits.
- **How to test it:** `golden_outputs_type_check`; the mod tests.

The engine's own tests write these files from real runs on the built fixture: `crates/planroom/tests/golden.rs` owns `cli/focus.json` and `cli/map-graph.json`. A plain `cargo xtask test` compares the engine's output with them; `cargo xtask gen --golden` rewrites them. `meta.took_ms` is left out, since no run repeats it.

## Layout

- `cli/`: one `--json` envelope per file, as the CLI prints it.
