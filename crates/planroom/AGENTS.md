# `planroom`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the CLI, the server, the live loop, app services (orchestration only), actor resolution, pack loading and embedding, config, the string catalog.
- **May depend on:** every workspace crate, `axum`, `tower-http`, `tokio`, `tokio-stream`, `tracing`, `anyhow`.
- **Must not:** business rules that belong in the kernel; spelling an error code outside the registry.
- **How to test it:** scenario tests `tests/sNN_*.rs`, `budgets.rs`, `actor.rs`, golden outputs.
