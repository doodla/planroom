# `planroom-store`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the tables, `commit` (idempotency, compare-and-set, status by actor), `tail`, leases, presence, layouts, the ledger, migrations.
- **May depend on:** `contract`, `rusqlite`.
- **Must not:** other workspace crates; business rules (it calls a kernel function passed in by `app`).
- **How to test it:** `concurrency.rs` (3 processes × 500 writes), `crash.rs`, `idem.rs`, `writer_interleavings` proptest, migration fixtures.
