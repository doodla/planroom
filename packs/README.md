# `packs/*`

This area's contract, from [Repo layout](../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** kinds, relations, hints, extractor and linker declarations, lenses, questions, rules, decision kinds, suggestions, prompts, stages, evals, pack tests.
- **May depend on:** the kernel's vocabulary and the `code` pack.
- **Must not:** code or executable files; other packs except `code`.
- **How to test it:** `tests/packs.rs` (validation, the inward-only rule); `planroom doctor --pack` on each; each pack's scenario tests.
