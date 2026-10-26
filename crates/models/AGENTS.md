# `planroom-models`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** `Transport` (live, record, replay), Jev codecs, the Claude gateway and session backends, retries, spend accounting.
- **May depend on:** `contract`, `reqwest`, `tokio`, `sha2`.
- **Must not:** other workspace crates; any network call in tests without `PLANROOM_LIVE_*`.
- **How to test it:** `jev_contract.rs` (both codecs over every fixture), `claude_decoders.rs`, `session_backend.rs`, `redact.rs`.
