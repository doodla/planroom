# `tests/golden`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the Rust tests' outputs that the mod tests and `tsc` read.
- **May depend on:** `cargo xtask gen --golden`.
- **Must not:** hand edits.
- **How to test it:** `golden_outputs_type_check`; the mod tests.

Until the engine's tests write these files, `cargo xtask gen` builds them from the contract types (`crates/xtask/src/golden.rs`), and `cargo xtask check` drift-checks them. They are stand-ins: `cli/focus.json` for the mod's tests, and `cli/map-graph.json` and `cli/map-empty.json`, `map --detail graph` on the fixture and on an unscanned workspace, for the canvas's e2e specs.
