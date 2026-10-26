# `planroom-contract`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** every public type, `ErrorCode`, `WarningCode`, `Command` and `CommandSpec`.
- **May depend on:** `serde`, `serde_json`, `schemars`, `clap` (derive only), `uuid`, `indexmap`.
- **Must not:** any other workspace crate; I/O; logic beyond validation.
- **How to test it:** `cargo xtask test -p planroom-contract` (schema snapshot, round trips).
