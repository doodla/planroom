# `planroom-kernel`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the fold, resolution, roll-up, the twelve query shapes and question pipelines, diff, impact, reconcile, policy bands, lint rule types, machine checks and step, budgets, next action, redaction, the decision and hand-off loop tables, roles.
- **May depend on:** `contract`, plus the allow-list in `boundaries.toml` (`serde`, `serde_json`, `petgraph`, `similar`, `nucleo-matcher`, `sha2`, `thiserror`, `indexmap`, `regex` for redaction).
- **Must not:** I/O, async, `rusqlite`, `reqwest`, `tree-sitter`; any pack kind ID as a string literal.
- **How to test it:** unit and property tests (`rollup`, `cas`, `undo`, `query`, `reconcile`, `evals`); `cargo bench fold`.
