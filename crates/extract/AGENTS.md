# `planroom-extract`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the `Extractor` and `Linker` traits, built-ins, the declarative runner, evidence location and verification, the git CLI adapter.
- **May depend on:** `contract`, `tree-sitter` and grammars, `ignore`, `globset`, `regex`, `saphyr`, `toml`, `pulldown-cmark`, `promql-parser`.
- **Must not:** other workspace crates; running anything from a scanned repo except `git` with hooks off.
- **How to test it:** `contract.rs` (every extractor over the fixture and a fuzz corpus), `link.rs`, per-extractor fixtures, the ABI floor test.
