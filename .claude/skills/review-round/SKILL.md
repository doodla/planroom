---
name: review-round
description: How the main agent runs one round of a Planroom phase review under KICKOFF's review protocol, from launching the lens reviewers and their refuters, through triaging the verdicts and fixing the material findings, to deciding whether another round runs. Use it at the end of every phase or milestone (architecture, walking skeleton, M1 to M5, the release's final review), whenever a review workflow's notification arrives, when you need to count or index findings, or when the question is "what's left after the review", even if the request only says "review it" or "the reviewers finished".
---

# Run a review round

KICKOFF's `<process>` makes every phase end with a review under one protocol. This skill is that protocol as Planroom runs it, with the tools that made it repeatable. Planroom has run it for the truth file, the architecture (three rounds) and the walking skeleton.

The protocol, from KICKOFF:

- Reviewers work at your model level or above. Each has a distinct lens and gets the work, not your reasoning.
- A separate verifier tries to refute each finding.
- You fix every material finding that survives refutation, and log each minor one as a known gap.
- There's no voting across lenses: one lens's material finding stands even if no other lens saw it.
- A review runs at most 3 rounds. What's left after the third is recorded as accepted risk.

## 1. Launch the round

1. Commit and push first. Name the commit in the reviewers' prompt so they review one fixed tree.
2. Copy `assets/review-workflow.js` into the Workflow tool's `script` and fill in its `<...>` parts:
   - `WORK`: the phase's scope, and what later milestones will build, so reviewers don't report planned work as missing.
   - `LENSES`: eight lenses that fail in different ways. The skeleton used correctness, security, drift, tests, repo design, the user's surfaces, operability and release, and expected-but-unasked. Adapt them to what the phase built.
3. Run two workflows of four lenses each. A workflow runs two agents at a time on this machine's 4 CPUs.
4. Keep `model: 'opus'` and an `effort` on every `agent()` call as literals; a hook refuses a call without them. Checkers are never weaker than what they check, so the refuters get the reviewers' model and effort.
5. Reviewers only read and run the built binary on a temporary fixture. They never build, write under the repo, or spend.

A round takes about 25 minutes of wall clock and 1.5 M subagent tokens per workflow.

## 2. Triage the verdicts

The notification's result is truncated. Read the journals, not the notification:

```sh
python3 -I .claude/skills/review-round/scripts/triage.py --round N \
  --out docs/contributing/research/<phase>-review \
  <transcript dir A>/journal.jsonl <transcript dir B>/journal.jsonl --rows <scratchpad>/rows.md
```

It writes `round<N>-<lens>.json` per lens, with each finding and its verdict, and prints each finding's outcome:

- `MATERIAL`: the verifier didn't refute it and kept it material. You fix it.
- `minor`: not refuted, or downgraded. It's a known gap.
- `refuted`: dropped.
- `UNVERIFIED`: no verdict yet. A refuter is missing or still running, so wait.

A verifier's `severity_after` is final. It can downgrade a finding or upgrade it. Don't overrule it in either direction.

Then write `docs/contributing/research/<phase>-review/README.md`: how the round ran, the counts, the fix tracks, and the outcome table from `--rows`. Rerun with `--tracks tracks.json` (finding ID → track name) to fill in each material row's track.

## 3. Fix what's material

1. Read each material finding in full, with its `evidence`, `fix` and the verifier's `reason`. Group duplicates: two lenses often find one defect.
2. Sort each defect into one of two kinds:
   - **A defect in what exists:** fix the code.
   - **A gap with no milestone:** fix the plan. Add a slice row in `architecture.md`'s Milestones naming the work, and fix any doc that claims it's done. Make the gap visible where it bites too, as `check`'s `not built (M1)` lines do.
3. Group the code fixes into tracks by area: engine, contract, a surface, the plugin. Each track runs in its own worktree, built by `opus` at high effort. Brief each track with:
   - the finding IDs, and the path of the round's JSON files;
   - reproduce each finding first, then show it fixed;
   - the design docs are the spec, so report deviations as doc conflicts rather than editing `docs/contributing/*.md`;
   - which other tracks run beside it, so it stays out of their way;
   - the disk budget.
4. Before launching worktrees, check the disk. Each worktree's `target/` takes 2 to 4 GB even with line tables only. Four worktrees is the most this disk holds.
5. Do small items yourself in the main checkout while the tracks run.
6. Merge each track with the `merge-track` skill. Write one decision record for the round's fixes, covering every doc conflict and every contract addition the tracks reported.

## 4. Decide on the next round

Run round N+1 when round N had any material finding. Its reviewers see the fixed tree, and its `WORK` names round N's README so they check that each fix holds. A round with no material finding ends the review.

After round 3, don't run a fourth. Write `accepted-risk.md` beside the README. List each material finding still open, why it's accepted, and who signed it off: the main agent, or the owner when it touches their data, money or credentials.

## 5. Record it

- Add a log entry to `progress.md`: the round's counts, the fix commits, the seam bugs, and the decision record.
- Update the "Where I am" section of `.claude/carry.md`.
- Each known gap stays in the round's README. When a milestone closes one, change its outcome there rather than deleting the row.
