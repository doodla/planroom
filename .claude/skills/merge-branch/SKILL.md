---
name: merge-branch
description: How you squash-merge a Planroom branch that an agent built in parallel in a worktree into the working branch, verify it, ratify gated surfaces with a decision record and the layout lock, push, and get CI green. Use it whenever a Workflow tool run or an agent that built a branch in a worktree (a worktree-* branch, or a named one such as fix/import-budget) reports back, when two such branches need merging one after another, when a merge hits conflicts in generated files, or when CI turns red after a merge, even if the request only says "merge it" or "the agent finished".
---

# Merge a branch built in parallel

You merge the branch and push it yourself; the agent that built it doesn't (KICKOFF `<process>`, `AGENTS.md`). Its report is a claim, not proof: everything below re-checks it on the merged tree. Seam bugs live where two branches meet, and every one so far was invisible to each branch alone. Examples: the person-token encoding, the mod's plugin root, a skill named like a mod command.

A branch is named for what it changes, such as `fix/import-budget` or `feat/pack-loader`. Each one lands as one squash commit on the working branch.

## Before merging

1. **Read the agent's whole report**, not the truncated notification. When the Workflow tool built the branch, `journal.jsonl` in that run's transcript directory holds each agent's full result. Note three lists from it: what it verified, its doc conflicts, and the gated surfaces it changed.
2. **Free disk first.** A worktree's `target/` is 3 to 8 GB, and a full disk fails builds halfway through. Run `df -h /`. Delete the build directories of merged or finished worktrees (`rm -rf .claude/worktrees/*/target`), never their branches.
3. **Preview the conflicts:** `git merge-tree --write-tree --name-only <working-branch> <branch>`. Merge branches one at a time, in the order the plan names; the second branch merges onto the first's result.

## Merging

1. `git merge --squash <branch>`. It stages the branch's changes as one change and doesn't commit.
2. **Generated files never merge by hand.** Take either side, then run the generator that the Generated directories table in `docs/contributing/repo-layout.md` names: `cargo xtask gen` for most (the schemas, the TypeScript contract, the mod bundle `plugin/hooks/planroom-mod.js`, `plugin.json`'s content-stamped version, `engine.env`, the area contract blocks), and `cargo xtask gen --golden` for `tests/golden/`. A `layout.lock` conflict is yours to resolve: rewrite it with `gen --layout-lock`, never by hand.
3. **Source conflicts:** keep both sides' intent. Usually both branches added an item to the same list: a usage line, a match arm, a contract re-export. Union them. When both branches fixed the same bug differently, keep one fix and drop the other.
4. **New dependencies:** run `pnpm install --frozen-lockfile --offline`, and without `--offline` if that fails. Otherwise `tsc` and the bundle fail with "cannot find module" errors that look like code bugs. The xtask commands don't run the install themselves yet (an M1 gates item); once they do, this step goes.
5. **Version-coupled pins:** the workspace path dependencies carry `version = "=X.Y.Z[-dev]"`. If the branch changed the workspace version, update those pins to match, or `cargo metadata` fails.

## Ratify what changed

1. Run `cargo xtask check layout`. A changed gated surface fails it on the main checkout. The gated surfaces are `boundaries.toml`, the crate graph, the top-level directories, each package's exports and dependency names, the area contracts table, and `AGENTS.md`'s Boundaries and Conventions sections.
2. **Every doc conflict and every changed gated surface needs a decision:**
   - Write one decision record for the merge, in the shape of DR-81 to DR-84: Status, Starting point, Origin, Context, Decision, Alternatives rejected, Consequences, Sources. Add its index row.
   - Decide each conflict yourself. Either the code follows the docs, with the milestone named, or the docs follow the code because the code is right.
   - Then apply the decision to its homes: `contracts.md`, `architecture.md`, `repo-layout.md`, `feature_list.json`.
3. For a gated change, run `cargo xtask gen --layout-lock DR-n`, naming the record you just wrote. The record and the new `layout.lock` go in the squash commit.

## Verify on the merged tree

Run the full suite yourself and read the summaries:

- `cargo xtask check`: every step `ok`. Locally, only `cargo deny` may be skipped; CI runs it.
- `cargo xtask test`: canvas build, `cargo test`, vitest and `claude plugin test target/mod-test` all `ok`. The last needs the full command, which rebuilds the harness; a direct `claude plugin test` reuses the old bundle.
- `cargo xtask test --installer` when `plugin/scripts/` or `tests/installer/` changed. It runs under sh, dash and BusyBox.
- `cargo xtask e2e` when the canvas, `serve` or `share` changed.

Chain commit and push so they run only on success. A shell `;` after a failing check still pushes, and a red push costs a CI cycle.

## Commit and push

- Commit the squashed change as one commit. Its message says what changed and why, in Planroom's terms: the behavior, the contract or the docs, not the branch's name, the agents' roles or how the review ran. It includes the decision record and the `gen --layout-lock` output. End it with the session's `Co-Authored-By` and `Claude-Session` trailers.
- If another agent is editing the same working tree, `git add` your own paths only, never `-A`.
- Push with `git push -u origin <working-branch>`. Never open a pull request, push a tag or change repository settings without the owner.
- **Clean up:** run `git worktree remove --force` for the branch's worktree. Then delete the branch with `git branch -D <branch>`: `-d` refuses a squash-merged branch, because its commits aren't in your history, so delete it only after the squash commit is pushed. Delete the leftover `worktree-*` branch, which holds only the worktree's starting commit, with `-D` too.

## Get CI green

CI catches what this container can't: cargo-deny, a BusyBox that prefers its own applets, macOS's `/private/var` paths, and slow runners. Read it through the GitHub MCP tools:

1. `mcp__github__actions_list` with `list_workflow_runs` and `workflow_runs_filter: {branch}`. A newer push cancels older runs, so read the newest completed one.
2. `list_workflow_jobs` on that run, then `mcp__github__get_job_logs` with `return_content` and a `tail_lines` large enough to show the failing test.
3. Reproduce the failure locally where you can, fix the root cause, and push. Don't retry, skip or quarantine a test to get green.

Nothing wakes the session when CI finishes. Schedule a check-in with `send_later` if you won't otherwise come back.

Don't push again until the merge's run has finished. A newer push cancels the older run, so a stream of notes and docs pushes after a merge can leave every merge commit without a finished run, and CI red for hours with nobody seeing it. Read the newest completed run, not the newest run, and treat `cancelled` as not yet checked.

## Record it

- Add a dated entry to `progress.md` with the commits, what you verified, the seam bugs found, and the decisions made.
- Update the notes file that `CLAUDE.md`'s "Keeping track across compactions" names, keeping it under 4,000 characters.
