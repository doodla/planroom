# `planroom-testkit`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the fixture wrapper (`examples/build-fixture` plus the workspace config), the truth comparator and its planted-name mapping, the S8 load harness and fault injection, golden helpers.
- **May depend on:** `contract`, `assert_cmd`, `tempfile`, `serde_json`.
- **Must not:** being a normal dependency of any crate (dev-only).
- **How to test it:** its own unit tests; every scenario test uses it.
