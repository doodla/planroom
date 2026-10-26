---
name: review-round
description: How you run one round of a Planroom phase review under KICKOFF's review protocol, from launching reviewers with distinct focuses and a verifier for each, through triaging the verdicts and fixing the material findings, to deciding whether another round runs. Use it at the end of every phase or milestone (architecture, walking skeleton, M1 to M5, the release's final review), whenever a review run's notification arrives, when you need to count or index findings, or when the question is "what's left after the review", even if the request only says "review it" or "the reviewers finished".
---

# Run a review round

KICKOFF's `<process>` makes every phase end with a review under one protocol. This skill is that protocol as Planroom runs it, with the tools that made it repeatable. Planroom has run it for the truth file, the architecture (three rounds) and the walking skeleton.

The protocol, from KICKOFF:

- Reviewers work at your model level or above. Each has a distinct focus and gets the work, not your reasoning.
- A separate verifier tries to refute each finding.
- You fix every material finding that survives verification, and log each minor one as a known gap.
- There's no voting across focuses: one focus's material finding stands even if no other focus saw it.
- A review runs at most 3 rounds. What's left after the third is recorded as accepted risk.

## 1. Launch the round

1. Commit and push first. Name the commit in the reviewers' prompt so they review one fixed tree.
2. Copy `assets/review-workflow.js` into the Workflow tool's `script` and fill in its `<...>` parts:
   - `WORK`: the phase's scope, and what later milestones will build, so reviewers don't report planned work as missing.
   - `FOCUSES`: eight focuses that fail in different ways. The skeleton used correctness, security, drift, tests, repo design, the user's surfaces, operability and release, and expected-but-unasked. Adapt them to what the phase built.
3. Run the reviewers in two batches of four focuses, one Workflow tool run per batch. A run executes two agents at a time on this machine's 4 CPUs.
4. Keep `model: 'opus'` and an `effort` on every `agent()` call as literals; a hook refuses a call without them. Checkers are never weaker than what they check, so the verifiers get the reviewers' model and effort.
5. Reviewers only read and run the built binary on a temporary fixture. They never build, write under the repo, or spend.

A round takes about 25 minutes of wall clock and 1.5 M subagent tokens per batch.

## 2. Triage the verdicts

The notification's result is truncated. Read the journals, not the notification:

```sh
python3 -I .claude/skills/review-round/scripts/triage.py --round N \
  --out docs/contributing/research/<phase>-review \
  <transcript dir A>/journal.jsonl <transcript dir B>/journal.jsonl --rows <scratchpad>/rows.md
```

It writes `round<N>-<focus>.json` per focus, with each finding and its verdict, and prints each finding's outcome:

- `MATERIAL`: the verifier didn't refute it and kept it material. You fix it.
- `minor`: not refuted, or downgraded. It's a known gap.
- `refuted`: dropped.
- `UNVERIFIED`: no verdict yet. A verifier is missing or still running, so wait.

A verifier's `severity_after` is final. It can downgrade a finding or upgrade it. Don't overrule it on judgment in either direction. One check is yours, though: before you drop a refuted security or data-loss finding, rerun its repro yourself, since a wrong refutation there costs the most. If it still reproduces, the finding stands as the reviewer rated it.

A "completed" notification doesn't mean every verifier returned. The script counts a finding with no verdict as `UNVERIFIED`. Rerun only that focus's verifier on the same commit: that finishes the round, and doesn't start a new one. Check too that every focus you launched has a `review:` line; a focus that never ran is not a clean focus.

Then write `docs/contributing/research/<phase>-review/README.md`: how the round ran, the counts, the fix branches, and the outcome table from `--rows`. Rerun with `--fixes fixes.json` (finding ID → branch name) to fill in each material row's fix branch.

## 3. Fix what's material

1. Read each material finding in full, with its `evidence`, `fix` and the verifier's `reason`. Group duplicates: two focuses often find one defect.
2. Sort each defect into one of two kinds:
   - **A defect in what exists:** fix the code.
   - **A gap with no milestone:** fix the plan. Add a slice row in `architecture.md`'s Milestones naming the work, and fix any doc that claims it's done. Make the gap visible where it bites too, as `check`'s `not built (M1)` lines do.
3. Group the code fixes into branches by area: engine, contract, a surface, the plugin. Name each branch for what it fixes, such as `fix/import-budget`. Each branch is built in its own worktree by an agent (the Agent tool, or the Workflow tool) on `opus` at high effort. Brief each agent with:
   - the finding IDs, and the path of the round's JSON files;
   - reproduce each finding first, then show it fixed;
   - the design docs are the spec, so report deviations as doc conflicts rather than editing `docs/contributing/*.md`;
   - which other branches are being built beside it, so it stays out of their way;
   - the disk budget.
4. Before launching worktrees, check the disk. Each worktree's `target/` takes 2 to 4 GB even with line tables only. Four worktrees is the most this disk holds.
5. Do small items yourself in the main checkout while the agents work.
6. Merge each branch with the `merge-branch` skill. Write one decision record for the round's fixes, covering every doc conflict and every contract addition the agents reported.
7. Run the full suite once on the tree with every branch merged. Seams between branches show up only there: one branch's regenerated golden broke another's test fixture, and one branch's new test refused another's new code.
8. Apply the record to its home pages with one docs agent in a worktree, then have an independent agent check those doc edits against the code. Fix what it finds before the next round starts, so the next round's reviewers read docs that match the code. Each such check has found real defects the fix branches left, in code as well as docs.

## 4. Decide on the next round

Run round N+1 when round N had any material finding. Its reviewers see the fixed tree, and its `WORK` names round N's README so they check that each fix holds. A round with no material finding ends the review.

Build round N+1's scripts from round N's, so the focuses stay the same: copy the two scripts, change the `name`, `description`, commit and record range, and replace only the round paragraph in `WORK`. That paragraph asks each reviewer to say, per material ID in its focus, whether the fix holds, then to look for what the fixes introduced, and not to re-report a logged minor gap unless it got worse. Rebuild `target/debug/planroom` first, so reviewers run the code they read.

After round 3, don't run a fourth. Write `accepted-risk.md` beside the README. List each material finding still open, why it's accepted, and who signed it off: you, or the owner when it touches their data, money or credentials. Round 3's own fixes get no reviewer either. Say so there, and say what checks them instead: each fix's regression test, the finding's original repro rerun, and the full suite.

## 5. Record it

- Add a log entry to `progress.md`: the round's counts, the fix commits, the seam bugs, and the decision record.
- Update the notes file that `CLAUDE.md`'s "Keeping track across compactions" names.
- Each known gap stays in the round's README. When a milestone closes one, change its outcome there rather than deleting the row.
