# Architecture review round 1: known gaps

Findings the round 1 fix pass left open, one row each, with the reason each stays a gap.

| ID | Title | Why it stays a gap |
|---|---|---|
| extensibility-6 | Linkers are a closed Rust seam, so pack-declared hint kinds can't be joined without engine changes | A generic declarative `join` linker is new design scope, and DR-9 records the limit; the S10 meaning of "core" is now defined (DR-13). |
| extensibility-7 | The `code` pack is a privileged, unversioned base that out-of-tree packs depend on implicitly | Versioned `requires` and generated code-pack reference tables are a contract change no scenario needs in v0.1. |
| extensibility-9 | Surfaces are outside the domain-free rule, and packs can't say how new kinds and lenses look | Kind and lens style slots, a surface literal scan and full-lens snapshots belong to the surfaces design, not this group of fixes. |
| security-10 | The engine checksum comes from the same release it checks, and the source fallback pins a mutable tag | Hashes and a source commit in `engine.env` need a two-commit release-prep flow and cross-runner reproducible archives that the release design doesn't have yet; `SECURITY.md` states the limit. |
| cost-9 (part) | Replay keys churn on every contract or prompt change | The live spend plan, the estimate and the refusal in `xtask live record` are in; a check that freezes model-facing schemas before recording waits until the contracts settle in M1. |
| cost-12 | 300 KB of contributor docs, with facts repeated | De-duplicating the tool and config tables across pages and adding a duplicate-table docs test is an editorial pass over every page; this pass only made Spend guards link to Configuration. |
| principles-7 (part) | Ghost-only writes still go through permission prompts | `room_change` now auto-allows its ghost-only leaves; pre-allowing the sync's `ArtifactData` calls would change the person's Claude Code permission settings, which DR-31 rules out. |
| principles-10 (part) | Agent defaults are not agent-first | The tools' `focus` summary and the re-run handle are in; JSON by default on a non-terminal would change the CLI's global flags (DR-22) for a case the skills cover by always passing `--json`. |
| unasked-12 | Repo identity is the directory name, so a rename or re-clone orphans every correction | A stable repo identity changes how IDs are minted (DR-42) and needs a rule for forks that share a root commit; until then a rename's orphaned overrides batch into one `override_retarget` card per kind (DR-50). |
| unasked-10 (part) | Accessibility is thin beyond contrast | An axe-core audit, a canvas keyboard model and ARIA roles for blocks, edges and cards need their own design pass in M5's accessibility slice; the contrast bar, its token test and the tape glyphs are in. |
| unasked-9 (part) | No uninstall command | `doctor --clean` would add a flag (R14); the "Remove Planroom" guide lists what to delete, and cancel and drop now archive the branch and remove the worktrees (DR-55). |
| unasked-7 (part) | No repo picker for a directory with many repos | A picker is new surface; nothing is created or scanned until someone runs `scan`, and `scan PATH` maps any one repo (DR-57). |
| unasked-8 (part) | No WSL 2 guide page | The Windows line points to WSL 2 (DR-58); a page on keeping repos on the Linux filesystem belongs to M5's docs pass. |
| operability-11 (part) | The fold benchmark is still a wall-clock budget | An instruction-count budget needs a counting harness the design doesn't have; the bench now gates on ubuntu-24.04 only (DR-43). |
| ux-7 (part) | `/room sync` doesn't queue its model turn with `$.prompt.submit` | Whether a typed slash command's returned context starts the model's turn is unverified, so the fix waits for WS8's sync check. |
| repo-8 (part) | Only the kernel has a full external allow-list | Allow-lists for every crate would freeze dependencies before the code exists; `store` and `extract` now forbid network clients and the kernel's I/O ban is a lint (DR-59), and the other "may depend on" columns become allow-lists once M1 settles them. |
