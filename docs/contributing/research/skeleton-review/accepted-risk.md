# The walking skeleton's review: accepted risk

This page is for whoever picks up the skeleton after its review, and for the owner, who signs off on what touches their data, money or credentials. The review protocol stops at round 3. What round 3 left open is recorded here as accepted risk: what it is, why it's accepted, what checks it now, and who signed it off. Each finding's outcome is in [the review's index](README.md), and the decisions are in [DR-87](../../decisions.md#dr-87-what-the-walking-skeletons-review-settled-in-round-3).

## Round 3's fixes had no reviewer

No fourth round runs, so no reviewer read round 3's fixes. Three checks stand in for one:

- **A regression test per finding.** Each track wrote a test that failed on the old code, then fixed the code until it passed.
- **The finding's own repro, rerun.** Each track ran the reviewer's repro before and after the fix and kept both outputs.
- **The full suite on the merged tree.** It ran once with both tracks merged, where seams between tracks show up, and CI ran again on the pushed commit.

| Finding | Regression test | Repro, before and after |
| --- | --- | --- |
| correctness-r3-1 | `crates/planroom/tests/scan.rs::a_scan_that_read_an_older_repo_set_never_moves_a_repo_back`; `crates/store/tests/commit.rs::a_scan_learns_when_another_scan_moved_the_repo_after_it_began` | before: the late scan moved all 5 repos back; after: it writes nothing and warns `scan_superseded` with `repos_changed` 5 times |
| unasked-r3-2 (with correctness-r3-3) | the transport plants in `crates/extract/src/git.rs`, each with a control that plain `-c protocol.allow=never` fails; `crates/planroom/tests/git_hardening.rs` (an `ext::` partial clone, a valueless `promisor`) | before: the repo's `ext::` and `uploadpack` commands ran; after: neither runs, and the valueless key maps with `partial_scan` |
| ux-r3-1 (with correctness-r3-2) | `crates/planroom/tests/scan.rs::a_workspace_whose_only_repo_has_no_commit_says_to_commit_and_never_loops` | before: "Map these 1 repos?" and a scan that mapped nothing, forever; after: `repos_unborn` at every level, before and after a scan |
| drift-3-1 | `crates/planroom/tests/budgets.rs` (import with 40 rows); `crates/planroom/tests/s11_remote.rs::a_large_sync_counts_its_rows_and_its_batch_files_hold_every_write` | before: 3,442 tokens over import's 800; after: 320 tokens for 40 rows |
| security-r3-1 | `packages/mod/src/guard.test.ts` (the shell's directory); `packages/mod/plugin-tests/guard.test.tsx` (two calls with a `cd` between) | before: `cat serve.json` after `cd .planroom` passed; after: denied |
| security-r3-2 | `guard.test.ts` (every tool's paths, uploads); `packages/mod/plugin-tests/guard.test.tsx` | before: ArtifactData, Artifact, SendUserFile, LS and an unknown MCP tool read the guarded files; after: all denied, and share's and import's own calls still pass |
| security-r3-3 | `guard.test.ts` (11 new write bypasses); `packages/mod/plugin-tests/guard.test.tsx` | before: Write and Bash rewrote `scripts/planroom` and the mod bundle; after: denied, and the skill's own `scripts/planroom` call still runs |
| drift-3-2 | `packages/mod/src/engine.test.ts` (an unknown reason) | removing one case from the switch fails `tsc -b` |
| ops-r3-1 | the macOS job itself, which failed SCAN2's test on every run | green on macOS since `a756f22` |

The repro outputs for the guard are in the GUARD3 track's report. The suite's results on the merged tree are in `progress.md`'s log entry for round 3.

## What stays open

| Open item | Why it's accepted | What checks it now | Closed by | Signed off by |
| --- | --- | --- | --- | --- |
| **Intermittent Ubuntu CI failures** (ops-r3-1's second half). Before the fixes, Ubuntu failed a different test each run (`budgets`, the live-watch test); none reproduced locally. | Its cause is unknown. Run 86 was green with the same tests, and a failing step now keeps its last 400 lines with nextest's `--failure-output final`, so the next failure names its cause. | Every CI run. A red Ubuntu run is root-caused from its log, never rerun to green. | the first M1 wave that sees it recur | main agent |
| **The guard's residual risks** (round 3's GUARD3, on top of DR-85's and DR-86's). If a host doesn't report the shell's directory, a `cd` to a parent of a guarded place followed by a relative path in a later call passes. A tool the guard doesn't know is held only through strings that look like paths: bare relative names and paths with spaces aren't seen, and only the read rule applies. Sibling plugin versions in the cache and the marketplace clone aren't write-protected. A program that writes through a bare relative name inside the plugin folder passes. DR-85's and DR-86's: scripts run by path, a variable set in an earlier call, `dotglob`, programs that walk or write files themselves, a tree read rooted at or above the workspace, a crafted tree copied above the data root, quoted text that reads as a redirection. | The guard is a guardrail, not a sandbox (architecture.md, Actors). An agent can already read the store through `planroom`; the person key never leaves the mod's store, and a person action needs its one-time signed token. Closing these needs the host's sandbox, not more patterns. | The guard's bypass and everyday lists in `guard.test.ts` and `packages/mod/plugin-tests/guard.test.tsx`; each new bypass found joins them. | `SECURITY.md` names them (M5) | main agent; **owner's confirmation owed**, since they touch the person key and the gateway key |
| **`$.session.cwd()` follows the shell's `cd`** is read from the host's type docs, not seen live. | The refusal of a `cd` into a guarded place covers the guarded places if the host doesn't report the move. | M1's live mod run; `packages/mod/plugin-tests/guard.test.tsx` uses a stand-in. | M1 (the mod's live run) | main agent |
| **A late scan whose inputs are stale, with no competing scan, still commits.** | It isn't a move back: the next scan corrects it, and a competing scan, the case that moved repos back, is refused. | `scan.rs`'s late-scan tests. | M1's linkers, which move the cross-repo join out of each repo's key (DR-86) | main agent |
| **Transports outside the nine named** (`foo::url`) are refused only if no `git-remote-foo` helper is on `PATH`. | A repo's config can't put a helper on `PATH`, and the cleared environment holds no repo-chosen `PATH`. | `git.rs`'s transport plants. | A-21's git 2.34.1 CI job checks the oldest supported git (M1) | main agent |
| **An old git without `GIT_NO_LAZY_FETCH`** was simulated by removing the variable on git 2.43, not run. | The per-protocol flags close the bypass without the variable, and the plant tests show it on the git CI runs. | `git.rs`'s `AS_OLD_GIT` tests. | A-21's git 2.34.1 CI job (M1) | main agent |
| **`repos_unborn` on the surfaces.** The canvas and pane were read, not run, in this state; no e2e test covers it. | Their empty-state press runs `focus` through the path `repos_unreadable` already uses, which e2e covers. | `scan.rs`'s unborn test at every level of `focus` and `map`. | M1's S1 surfaces work | main agent |
| **The 40 minor findings of round 3**, and the minor findings of rounds 1 and 2 not yet closed. | Each is a known gap in its area's milestone. None loses data, spends money or exposes a credential. | The index lists each with its outcome. | each area's milestone; the index changes the row when one closes | main agent |

## Sign-off

- **Main agent**, 2026-10-26: every item above.
- **Owner**: the guard's residual risks, since they touch the person key and the gateway key. The final report asks for this confirmation, and `SECURITY.md` (M5) states them to users either way.
