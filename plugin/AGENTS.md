# `plugin/`

This area's contract, from [Repo layout](../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the shipped plugin.
- **May depend on:** built outputs and hand-written skills, agents, resolver.
- **Must not:** `bin/`; symlinks; hand-written copies of generated files.
- **How to test it:** `claude plugin validate --strict`; `tests/plugin/`; `tests/installer/`.
