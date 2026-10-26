---
name: merge-track
description: How the main agent merges a Planroom track branch built in a worktree into the working branch, verifies it, ratifies gated surfaces with a decision record and the layout lock, pushes, and gets CI green. Use it whenever a workflow or agent that built a track (track/*, a worktree-* branch) reports back, when two tracks need merging one after another, when a merge hits conflicts in generated files, or when CI turns red after a merge, even if the request only says "merge it" or "the agent finished".
---

# Merge a track

In Planroom only the main agent merges tracks and pushes (KICKOFF `<process>`, `AGENTS.md`). A track's report is a claim, not proof: everything below re-checks it on the merged tree. Seam bugs live where tracks meet, and every one so far was invisible to each track alone. Examples: the person-token encoding, the mod's plugin root, a skill named like a mod command.

## Before merging

1. **Read the track's whole report**, not the truncated notification: `journal.jsonl` in the workflow's transcript directory holds each agent's full result. Note three lists from it: what it verified, its doc conflicts, and the gated surfaces it changed.
2. **Free disk first.** A worktree's `target/` is 3 to 8 GB, and a full disk fails builds halfway through. Run `df -h /`. Delete the build directories of merged or finished worktrees (`rm -rf .claude/worktrees/*/target`), never their branches.
3. **Preview the conflicts:** `git merge-tree --write-tree --name-only <working-branch> track/<name>`. Merge tracks one at a time, in the order the plan names; the second track merges onto the first's result.

## Merging

1. `git merge --no-ff --no-commit track/<name>`.
2. **Generated files never merge by hand.** Take either side, then run the generator that the Generated directories table in `docs/contributing/repo-layout.md` names: `cargo xtask gen` for most (the schemas, the TypeScript contract, the mod bundle `plugin/hooks/planroom-mod.js`, `plugin.json`'s content-stamped version, `engine.env`, the area contract blocks), and `cargo xtask gen --golden` for `tests/golden/`. A `layout.lock` conflict is yours to resolve: rewrite it with `gen --layout-lock`, never by hand.
3. **Source conflicts:** keep both sides' intent. Usually both tracks added an item to the same list: a usage line, a match arm, a contract re-export. Union them. When both tracks fixed the same bug differently, keep one fix and drop the other.
4. **New dependencies:** run `pnpm install --frozen-lockfile --offline`, and without `--offline` if that fails. Otherwise `tsc` and the bundle fail with "cannot find module" errors that look like code bugs. The xtask commands don't run the install themselves yet (an M1 gates item); once they do, this step goes.
5. **Version-coupled pins:** the workspace path dependencies carry `version = "=X.Y.Z[-dev]"`. If a track changed the workspace version, update those pins to match, or `cargo metadata` fails.

## Ratify what changed

1. Run `cargo xtask check layout`. A changed gated surface fails it on the main checkout. The gated surfaces are `boundaries.toml`, the crate graph, the top-level directories, each package's exports and dependency names, the area contracts table, and `AGENTS.md`'s Boundaries and Conventions sections.
2. **Every doc conflict and every changed gated surface needs a decision:**
   - Write one decision record for the merge, in the shape of DR-81 to DR-84: Status, Starting point, Origin, Context, Decision, Alternatives rejected, Consequences, Sources. Add its index row.
   - Decide each conflict yourself. Either the code follows the docs, with the milestone named, or the docs follow the code because the code is right.
   - Then fold the decision into its homes: `contracts.md`, `architecture.md`, `repo-layout.md`, `feature_list.json`.
3. For a gated change, run `cargo xtask gen --layout-lock DR-n` in the same merge commit, naming the record you just wrote.

## Verify on the merged tree

Run the full suite yourself and read the summaries:

- `cargo xtask check`: every step `ok`. Locally, only `cargo deny` may be skipped; CI runs it.
- `cargo xtask test`: canvas build, `cargo test`, vitest and `claude plugin test target/mod-test` all `ok`. The last needs the full command, which rebuilds the harness; a direct `claude plugin test` reuses the old bundle.
- `cargo xtask test --installer` when `plugin/scripts/` or `tests/installer/` changed. It runs under sh, dash and BusyBox.
- `cargo xtask e2e` when the canvas, `serve` or `share` changed.

Chain commit and push so they run only on success. A shell `;` after a failing check still pushes, and a red push costs a CI cycle.

## Commit and push

- Use one merge commit whose message says what the track added and what you verified. End it with the session's `Co-Authored-By` and `Claude-Session` trailers.
- If another agent is editing the same working tree, `git add` your own paths only, never `-A`.
- Push with `git push -u origin <working-branch>`. Never open a pull request, push a tag or change repository settings without the owner.
- **Clean up:** run `git worktree remove --force` for the track's worktree. Delete `track/<name>` with `git branch -d`, which refuses an unmerged branch, and the leftover `worktree-*` branch, which holds only the worktree's starting commit, with `-D`.

## Get CI green

CI catches what this container can't: cargo-deny, a BusyBox that prefers its own applets, macOS's `/private/var` paths, and slow runners. Read it through the GitHub MCP tools:

1. `mcp__github__actions_list` with `list_workflow_runs` and `workflow_runs_filter: {branch}`. A newer push cancels older runs, so read the newest completed one.
2. `list_workflow_jobs` on that run, then `mcp__github__get_job_logs` with `return_content` and a `tail_lines` large enough to show the failing test.
3. Reproduce the failure locally where you can, fix the root cause, and push. Don't retry, skip or quarantine a test to get green.

Nothing wakes the session when CI finishes. Schedule a check-in with `send_later` if you won't otherwise come back.

## Record it

- Add a dated entry to `progress.md` with the commits, what you verified, the seam bugs found, and the decisions made.
- Update `.claude/carry.md`'s "Where I am" section, keeping it under 4,000 characters.
