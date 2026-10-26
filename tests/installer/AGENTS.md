# `tests/installer`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the resolver's offline suite.
- **May depend on:** POSIX sh, a `127.0.0.1` release server.
- **Must not:** network access beyond loopback.
- **How to test it:** `cargo xtask test --installer` on ubuntu-24.04 and macos-15.
